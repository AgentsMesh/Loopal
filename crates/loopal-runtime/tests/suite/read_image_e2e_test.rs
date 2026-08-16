use loopal_provider::AnthropicProvider;
use loopal_runtime::hydrate::hydrate_turn_images;
use loopal_storage::FileResourceStore;
use loopal_tool_invocation::ToolImageBlock;
use tempfile::tempdir;

use crate::read_image_support::{
    MAX_IMAGE_BYTES, anthropic_params, minimal_png, persist_images, read_image,
    turn_with_tool_images,
};

#[tokio::test]
async fn small_image_full_pipeline_stays_inline() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let file = workdir.path().join("shot.png");
    std::fs::write(&file, minimal_png(32, 32)).unwrap();

    let result = read_image(workdir.path(), &file).await;
    assert!(!result.is_error);
    assert_eq!(result.images.len(), 1);

    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let images = persist_images(store.as_ref(), result.images).await;
    assert!(matches!(images[0], ToolImageBlock::Inline { .. }));

    let mut turns = vec![turn_with_tool_images("tu_1", images)];
    hydrate_turn_images(&mut turns, store.as_ref(), "e2e-session", MAX_IMAGE_BYTES)
        .await
        .unwrap();
    let messages =
        AnthropicProvider::new("k".into()).build_messages_json_from_turns(&anthropic_params(turns));
    let user = messages
        .iter()
        .rev()
        .find(|message| message["role"] == "user")
        .unwrap();
    let content = user["content"][0]["content"].as_array().unwrap();
    let image = content
        .iter()
        .find(|block| block["type"] == "image")
        .unwrap();
    assert_eq!(image["source"]["media_type"], "image/png");
}

#[tokio::test]
async fn invalid_path_returns_tool_error_without_panicking() {
    let workdir = tempdir().unwrap();
    let result = read_image(workdir.path(), &workdir.path().join("missing.png")).await;
    assert!(result.is_error);
    assert!(result.images.is_empty());
}
