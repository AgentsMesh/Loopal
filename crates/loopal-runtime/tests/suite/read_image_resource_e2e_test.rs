use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_runtime::hydrate::{hydrate_images, hydrate_turn_images};
use loopal_storage::FileResourceStore;
use loopal_tool_invocation::ToolImageBlock;
use loopal_turn::{ToolExecState, TurnStep};
use tempfile::tempdir;

use crate::read_image_support::{
    MAX_IMAGE_BYTES, padded_png, persist_images, read_image, turn_with_tool_images,
};

#[tokio::test]
async fn large_image_persists_and_hydrates_back() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let payload = padded_png(64, 64, 400 * 1024);
    let file = workdir.path().join("big.png");
    std::fs::write(&file, &payload).unwrap();

    let result = read_image(workdir.path(), &file).await;
    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let images = persist_images(store.as_ref(), result.images).await;
    let ToolImageBlock::SessionResource { id, byte_size, .. } = &images[0] else {
        panic!("expected session resource")
    };
    assert!(
        store_dir
            .path()
            .join("sessions/e2e-session/resources")
            .join(id)
            .exists()
    );
    assert_eq!(*byte_size, payload.len());

    let mut turns = vec![turn_with_tool_images("tu_1", images)];
    hydrate_turn_images(&mut turns, store.as_ref(), "e2e-session", MAX_IMAGE_BYTES)
        .await
        .unwrap();
    let TurnStep::ToolBatch(batch) = &turns[0].body.steps[1] else {
        panic!()
    };
    let ToolExecState::Done(result) = &batch.items[0].state else {
        panic!()
    };
    let ToolImageBlock::Inline { data, .. } = &result.images[0] else {
        panic!("expected hydrated inline image")
    };
    assert_eq!(STANDARD.decode(data).unwrap(), payload);
}

#[tokio::test]
async fn serialized_resource_hydrates_after_store_restart() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let payload = padded_png(48, 48, 300 * 1024);
    let file = workdir.path().join("image.png");
    std::fs::write(&file, &payload).unwrap();

    let result = read_image(workdir.path(), &file).await;
    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let images = persist_images(store.as_ref(), result.images).await;
    let message = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu_1".into(),
            content: String::new(),
            images,
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    };
    let encoded = serde_json::to_string(&message).unwrap();
    assert!(encoded.contains("\"type\":\"session_resource\""));
    let mut messages = vec![serde_json::from_str(&encoded).unwrap()];
    let reloaded = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    hydrate_images(
        &mut messages,
        reloaded.as_ref(),
        "e2e-session",
        MAX_IMAGE_BYTES,
    )
    .await
    .unwrap();
    let ContentBlock::ToolResult { images, .. } = &messages[0].content[0] else {
        panic!()
    };
    let ToolImageBlock::Inline { data, .. } = &images[0] else {
        panic!("expected hydrated inline image")
    };
    assert_eq!(STANDARD.decode(data).unwrap(), payload);
}

#[tokio::test]
async fn duplicate_reads_yield_same_resource_id() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let file = workdir.path().join("duplicate.png");
    std::fs::write(&file, padded_png(72, 72, 300 * 1024)).unwrap();
    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());

    let first = persist_images(
        store.as_ref(),
        read_image(workdir.path(), &file).await.images,
    )
    .await;
    let second = persist_images(
        store.as_ref(),
        read_image(workdir.path(), &file).await.images,
    )
    .await;
    let ToolImageBlock::SessionResource { id: first, .. } = &first[0] else {
        panic!()
    };
    let ToolImageBlock::SessionResource { id: second, .. } = &second[0] else {
        panic!()
    };
    assert_eq!(first, second);
}
