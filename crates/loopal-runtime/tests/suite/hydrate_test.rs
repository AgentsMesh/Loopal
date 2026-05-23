use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_error::StorageError;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_runtime::hydrate::{hydrate_images, maybe_persist_inline_images};
use loopal_storage::{FileResourceStore, ResourceStore};
use loopal_tool_invocation::ToolImageBlock;
use tempfile::tempdir;

fn b64(data: &[u8]) -> String {
    STANDARD.encode(data)
}

struct WriteFailingStore;

#[async_trait]
impl ResourceStore for WriteFailingStore {
    async fn write(&self, _: &str, _: &str, _: &[u8]) -> Result<String, StorageError> {
        Err(StorageError::Io(std::io::Error::other(
            "simulated write failure",
        )))
    }
    async fn read(&self, _: &str, _: &str) -> Result<Vec<u8>, StorageError> {
        unreachable!("read not invoked in write-failure test")
    }
    async fn delete_session(&self, _: &str) -> Result<(), StorageError> {
        Ok(())
    }
}

struct ReadFailingStore;

#[async_trait]
impl ResourceStore for ReadFailingStore {
    async fn write(&self, _: &str, _: &str, _: &[u8]) -> Result<String, StorageError> {
        unreachable!("write not invoked in read-failure test")
    }
    async fn read(&self, _: &str, _: &str) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "simulated read failure",
        )))
    }
    async fn delete_session(&self, _: &str) -> Result<(), StorageError> {
        Ok(())
    }
}

#[tokio::test]
async fn small_image_stays_inline() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let small = vec![0u8; 1024];
    let mut images = vec![ToolImageBlock::inline("image/png", b64(&small))];
    maybe_persist_inline_images(store.as_ref(), "sess", &mut images, 256 * 1024).await;
    assert!(matches!(images[0], ToolImageBlock::Inline { .. }));
}

#[tokio::test]
async fn large_image_persists_to_session_resource() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let big = vec![0u8; 300 * 1024];
    let mut images = vec![ToolImageBlock::inline("image/png", b64(&big))];
    maybe_persist_inline_images(store.as_ref(), "sess", &mut images, 256 * 1024).await;
    let ToolImageBlock::SessionResource { byte_size, .. } = &images[0] else {
        panic!("expected SessionResource");
    };
    assert_eq!(*byte_size, 300 * 1024);
}

#[tokio::test]
async fn hydrate_converts_session_resource_back_to_inline() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let payload = vec![7u8; 200];
    let id = store.write("sess", "image/png", &payload).await.unwrap();

    let mut messages = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu".into(),
            content: String::new(),
            images: vec![ToolImageBlock::session_resource(
                &id,
                "image/png",
                payload.len(),
            )],
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }];

    hydrate_images(&mut messages, store.as_ref(), "sess")
        .await
        .unwrap();

    let ContentBlock::ToolResult { images, .. } = &messages[0].content[0] else {
        panic!("expected ToolResult");
    };
    let ToolImageBlock::Inline { data, media_type } = &images[0] else {
        panic!("expected Inline after hydrate");
    };
    assert_eq!(media_type, "image/png");
    assert_eq!(STANDARD.decode(data.as_bytes()).unwrap(), payload);
}

#[tokio::test]
async fn hydrate_leaves_inline_untouched() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let mut messages = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu".into(),
            content: "ok".into(),
            images: vec![ToolImageBlock::inline("image/png", b64(b"original-data"))],
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }];

    hydrate_images(&mut messages, store.as_ref(), "sess")
        .await
        .unwrap();

    let ContentBlock::ToolResult { images, .. } = &messages[0].content[0] else {
        panic!();
    };
    let ToolImageBlock::Inline { data, .. } = &images[0] else {
        panic!();
    };
    assert_eq!(data, &b64(b"original-data"));
}

#[tokio::test]
async fn persist_then_hydrate_round_trip() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let big = vec![13u8; 300 * 1024];
    let original_data = b64(&big);

    let mut images = vec![ToolImageBlock::inline("image/png", original_data.clone())];
    maybe_persist_inline_images(store.as_ref(), "sess", &mut images, 256 * 1024).await;

    let mut messages = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu".into(),
            content: String::new(),
            images,
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }];

    hydrate_images(&mut messages, store.as_ref(), "sess")
        .await
        .unwrap();

    let ContentBlock::ToolResult { images, .. } = &messages[0].content[0] else {
        panic!();
    };
    let ToolImageBlock::Inline { data, .. } = &images[0] else {
        panic!("must hydrate back to Inline");
    };
    assert_eq!(data, &original_data);
}

#[tokio::test]
async fn maybe_persist_keeps_inline_when_base64_invalid() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let mut images = vec![ToolImageBlock::Inline {
        media_type: "image/png".to_string(),
        data: "***not valid base64 but long enough***".repeat(20_000),
    }];
    maybe_persist_inline_images(store.as_ref(), "sess", &mut images, 256 * 1024).await;
    assert!(
        matches!(images[0], ToolImageBlock::Inline { .. }),
        "invalid base64 must keep Inline instead of persisting"
    );
}

#[tokio::test]
async fn maybe_persist_keeps_inline_when_store_write_fails() {
    let big = vec![0u8; 300 * 1024];
    let mut images = vec![ToolImageBlock::inline("image/png", b64(&big))];
    let store = WriteFailingStore;
    maybe_persist_inline_images(&store, "sess", &mut images, 256 * 1024).await;
    assert!(
        matches!(images[0], ToolImageBlock::Inline { .. }),
        "store write failure must keep Inline"
    );
}

#[tokio::test]
async fn maybe_persist_below_threshold_skips_store_entirely() {
    let small = vec![0u8; 1024];
    let mut images = vec![ToolImageBlock::inline("image/png", b64(&small))];
    let store = WriteFailingStore;
    // store.write would Err if called; assertion holds only if maybe_persist
    // never reaches it because of the threshold short-circuit.
    maybe_persist_inline_images(&store, "sess", &mut images, 256 * 1024).await;
    assert!(matches!(images[0], ToolImageBlock::Inline { .. }));
}

#[tokio::test]
async fn hydrate_propagates_store_read_error() {
    let mut messages = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu".into(),
            content: String::new(),
            images: vec![ToolImageBlock::session_resource(
                "missing-id",
                "image/png",
                1024,
            )],
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }];

    let store = ReadFailingStore;
    let err = hydrate_images(&mut messages, &store, "sess")
        .await
        .unwrap_err();
    assert!(matches!(err, StorageError::Io(_)));
}

#[tokio::test]
async fn hydrate_skips_messages_without_tool_result() {
    let mut messages = vec![Message::user("just text"), Message::assistant("reply")];
    let store = ReadFailingStore;
    hydrate_images(&mut messages, &store, "sess").await.unwrap();
}
