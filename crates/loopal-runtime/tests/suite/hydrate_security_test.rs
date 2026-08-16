use loopal_error::StorageError;
use loopal_runtime::hydrate::{hydrate_images, maybe_persist_inline_images};
use loopal_storage::{FileResourceStore, ResourceStore};
use loopal_tool_invocation::ToolImageBlock;
use tempfile::tempdir;

use crate::hydrate_support::{MAX_IMAGE_BYTES, ReadFailingStore, b64, message, png};

#[tokio::test]
async fn persistence_rejects_malformed_and_mime_mismatched_inline_images() {
    for image in [
        ToolImageBlock::inline("image/png", "not-base64"),
        ToolImageBlock::inline("image/jpeg", b64(&png(32))),
        ToolImageBlock::inline("image/png", b64(b"not-an-image")),
    ] {
        let mut images = vec![image];
        let error =
            maybe_persist_inline_images(&ReadFailingStore, "sess", &mut images, 1, MAX_IMAGE_BYTES)
                .await
                .unwrap_err();
        assert!(matches!(error, StorageError::ResourceIntegrity));
    }
}

#[tokio::test]
async fn persistence_validates_entire_batch_before_writing() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let valid = ToolImageBlock::inline("image/png", b64(&png(300 * 1024)));
    let invalid = ToolImageBlock::inline("image/jpeg", b64(&png(32)));
    let mut images = vec![valid.clone(), invalid];
    let error = maybe_persist_inline_images(
        store.as_ref(),
        "sess",
        &mut images,
        256 * 1024,
        MAX_IMAGE_BYTES,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));
    assert_eq!(images[0], valid);
    assert!(!dir.path().join("sessions/sess/resources").exists());
}

#[tokio::test]
async fn hydrate_rejects_declared_size_mismatch() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let payload = png(64);
    let id = store.write("sess", "image/png", &payload).await.unwrap();
    let mut messages = vec![message(vec![ToolImageBlock::session_resource(
        id,
        "image/png",
        payload.len() + 1,
    )])];
    let error = hydrate_images(&mut messages, store.as_ref(), "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));
}

#[tokio::test]
async fn hydrate_rejects_mime_mismatch_and_tampered_disk_content() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let payload = png(64);
    let id = store.write("sess", "image/png", &payload).await.unwrap();
    let mut wrong_mime = vec![message(vec![ToolImageBlock::session_resource(
        &id,
        "image/jpeg",
        payload.len(),
    )])];
    let error = hydrate_images(&mut wrong_mime, store.as_ref(), "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));

    let path = dir.path().join("sessions/sess/resources").join(&id);
    std::fs::write(path, png(65)).unwrap();
    let mut tampered = vec![message(vec![ToolImageBlock::session_resource(
        id,
        "image/png",
        payload.len(),
    )])];
    let error = hydrate_images(&mut tampered, store.as_ref(), "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));
}

#[tokio::test]
async fn hydrate_failure_does_not_partially_replace_the_batch() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let payload = png(64);
    let id = store.write("sess", "image/png", &payload).await.unwrap();
    let first = ToolImageBlock::session_resource(&id, "image/png", payload.len());
    let invalid = ToolImageBlock::session_resource(id, "image/jpeg", payload.len());
    let mut messages = vec![message(vec![first.clone(), invalid])];
    let error = hydrate_images(&mut messages, store.as_ref(), "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));
    let loopal_provider_api::ContentBlock::ToolResult { images, .. } = &messages[0].content[0]
    else {
        panic!()
    };
    assert_eq!(images[0], first);
}

#[tokio::test]
async fn hydrate_rejects_image_count_and_total_byte_limits() {
    let inline = ToolImageBlock::inline("image/png", b64(&png(16)));
    let mut too_many = vec![message(vec![inline.clone(); 17])];
    let error = hydrate_images(&mut too_many, &ReadFailingStore, "sess", 1024)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));

    let mut too_large = vec![message(vec![inline.clone(), inline])];
    let error = hydrate_images(&mut too_large, &ReadFailingStore, "sess", 31)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        StorageError::ResourceByteLimitExceeded { max_bytes: 31 }
    ));
}

#[tokio::test]
async fn hydrate_propagates_bounded_store_read_failure() {
    let mut messages = vec![message(vec![ToolImageBlock::session_resource(
        "deadbeefdeadbeefdeadbeefdeadbeef",
        "image/png",
        16,
    )])];
    let error = hydrate_images(&mut messages, &ReadFailingStore, "sess", 1024)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::Io(_)));
}

#[tokio::test]
async fn persistence_rejects_resources_and_hydration_rejects_oversized_declarations() {
    let mut resources = vec![ToolImageBlock::session_resource(
        "already-persisted",
        "image/png",
        16,
    )];
    let error = maybe_persist_inline_images(&ReadFailingStore, "sess", &mut resources, 1, 1_024)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));

    let mut messages = vec![message(vec![ToolImageBlock::session_resource(
        "oversized",
        "image/png",
        1_025,
    )])];
    let error = hydrate_images(&mut messages, &ReadFailingStore, "sess", 1_024)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        StorageError::ResourceByteLimitExceeded { max_bytes: 1_024 }
    ));
}
