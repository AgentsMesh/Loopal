use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_message::{ContentBlock, Message, MessageRole, ToolImageBlock};
use loopal_provider::AnthropicProvider;
use loopal_provider_api::ChatParams;
use loopal_runtime::hydrate::{hydrate_images, maybe_persist_inline_images};
use loopal_storage::FileResourceStore;
use loopal_tool_api::{Tool, ToolContext, ToolDefinition, TypedBridge};
use loopal_tool_read_image::{ReadImageParams, ReadImageTool};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

fn minimal_png(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    v.extend_from_slice(&[0, 0, 0, 13]);
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v
}

fn padded_png(w: u32, h: u32, total_bytes: usize) -> Vec<u8> {
    let mut v = minimal_png(w, h);
    v.resize(total_bytes, 0);
    v
}

fn backend_for(cwd: &std::path::Path) -> Arc<LocalBackend> {
    LocalBackend::new(
        cwd.to_path_buf(),
        None,
        ResourceLimits::default(),
        "e2e-session",
    )
}

fn tool() -> TypedBridge<ReadImageTool, ReadImageParams> {
    TypedBridge::new(ReadImageTool)
}

fn anthropic_params(messages: Vec<Message>) -> ChatParams {
    ChatParams {
        model: "claude-test".to_string(),
        messages,
        turns: vec![],
        system_prompt: String::new(),
        tools: Vec::<ToolDefinition>::new(),
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

fn tool_result_block(tool_use_id: &str, images: Vec<ToolImageBlock>) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: tool_use_id.into(),
        content: String::new(),
        images,
        is_error: false,
        metadata: None,
    }
}

#[tokio::test]
async fn small_image_full_pipeline_stays_inline() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let png = minimal_png(32, 32);
    let file = workdir.path().join("shot.png");
    std::fs::write(&file, &png).unwrap();

    // Step 1: ReadImage tool execute → ToolResult
    let ctx = ToolContext::new(backend_for(workdir.path()), "e2e-session");
    let result = tool()
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(result.images.len(), 1);

    // Step 2: tool_exec bridge → ContentBlock + maybe_persist (under threshold)
    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let mut images = result.images;
    maybe_persist_inline_images(store.as_ref(), "e2e-session", &mut images, 256 * 1024).await;
    let block = tool_result_block("tu_1", images);

    let ContentBlock::ToolResult { images, .. } = &block else {
        panic!();
    };
    assert!(matches!(images[0], ToolImageBlock::Inline { .. }));

    // Step 3: provider serializes Inline directly
    let provider = AnthropicProvider::new("k".into());
    let mut messages = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![block],
        origin: None,
        ephemeral_in_history: false,
    }];
    hydrate_images(&mut messages, store.as_ref(), "e2e-session")
        .await
        .unwrap();
    let msgs = provider.build_messages(&anthropic_params(messages));
    let arr = msgs[0]["content"][0]["content"].as_array().unwrap();
    let image_block = arr.iter().find(|b| b["type"] == "image").unwrap();
    assert_eq!(image_block["source"]["media_type"], "image/png");
}

#[tokio::test]
async fn large_image_persists_to_resource_and_hydrates_back() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let png = padded_png(64, 64, 400 * 1024);
    let file = workdir.path().join("big.png");
    std::fs::write(&file, &png).unwrap();

    let ctx = ToolContext::new(backend_for(workdir.path()), "e2e-session");
    let result = tool()
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let mut images = result.images;
    maybe_persist_inline_images(store.as_ref(), "e2e-session", &mut images, 256 * 1024).await;

    let ToolImageBlock::SessionResource { id, byte_size, .. } = &images[0] else {
        panic!("large image must persist to SessionResource");
    };
    let stored = store_dir
        .path()
        .join("sessions/e2e-session/resources")
        .join(id);
    assert!(stored.exists(), "resource file must exist on disk");
    assert_eq!(*byte_size, png.len());

    let mut messages = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![tool_result_block("tu_1", images)],
        origin: None,
        ephemeral_in_history: false,
    }];

    // Simulate session reload + hydrate before next LLM call.
    hydrate_images(&mut messages, store.as_ref(), "e2e-session")
        .await
        .unwrap();
    let ContentBlock::ToolResult { images, .. } = &messages[0].content[0] else {
        panic!();
    };
    let ToolImageBlock::Inline { data, .. } = &images[0] else {
        panic!("hydrate must convert SessionResource back to Inline");
    };
    assert_eq!(STANDARD.decode(data.as_bytes()).unwrap(), png);
}

#[tokio::test]
async fn jsonl_round_trip_preserves_session_resource() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let png = padded_png(48, 48, 300 * 1024);
    let file = workdir.path().join("img.png");
    std::fs::write(&file, &png).unwrap();

    let ctx = ToolContext::new(backend_for(workdir.path()), "e2e-session");
    let result = tool()
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let mut images = result.images;
    maybe_persist_inline_images(store.as_ref(), "e2e-session", &mut images, 256 * 1024).await;
    let message = Message {
        id: None,
        role: MessageRole::User,
        content: vec![tool_result_block("tu_1", images)],
        origin: None,
        ephemeral_in_history: false,
    };

    // Serialize → deserialize: simulates session JSONL persistence.
    let line = serde_json::to_string(&message).unwrap();
    assert!(line.contains("\"type\":\"session_resource\""));
    let restored: Message = serde_json::from_str(&line).unwrap();

    let store_reload = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let mut messages = vec![restored];
    hydrate_images(&mut messages, store_reload.as_ref(), "e2e-session")
        .await
        .unwrap();

    let ContentBlock::ToolResult { images, .. } = &messages[0].content[0] else {
        panic!();
    };
    let ToolImageBlock::Inline { data, .. } = &images[0] else {
        panic!("must hydrate after restart");
    };
    assert_eq!(STANDARD.decode(data.as_bytes()).unwrap(), png);
}

#[tokio::test]
async fn duplicate_reads_yield_same_resource_id() {
    let workdir = tempdir().unwrap();
    let store_dir = tempdir().unwrap();
    let png = padded_png(72, 72, 300 * 1024);
    let file = workdir.path().join("dup.png");
    std::fs::write(&file, &png).unwrap();

    let store = FileResourceStore::with_base_dir(store_dir.path().to_path_buf());
    let ctx = ToolContext::new(backend_for(workdir.path()), "e2e-session");

    let mut first = tool()
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap()
        .images;
    maybe_persist_inline_images(store.as_ref(), "e2e-session", &mut first, 256 * 1024).await;
    let ToolImageBlock::SessionResource { id: id1, .. } = &first[0] else {
        panic!();
    };

    let mut second = tool()
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap()
        .images;
    maybe_persist_inline_images(store.as_ref(), "e2e-session", &mut second, 256 * 1024).await;
    let ToolImageBlock::SessionResource { id: id2, .. } = &second[0] else {
        panic!();
    };

    assert_eq!(
        id1, id2,
        "identical content must produce identical resource id"
    );
}

#[tokio::test]
async fn invalid_path_returns_tool_error_without_panicking() {
    let workdir = tempdir().unwrap();
    let ctx = ToolContext::new(backend_for(workdir.path()), "e2e-session");
    let missing = workdir.path().join("nope.png");
    let result = tool()
        .execute(json!({"file_path": missing.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.images.is_empty());
}
