use std::path::Path;
use std::sync::Arc;

use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_provider_api::ChatParams;
use loopal_runtime::hydrate::maybe_persist_inline_images;
use loopal_storage::ResourceStore;
use loopal_tool_api::{Tool, ToolContext, ToolDefinition, ToolResult, TypedBridge};
use loopal_tool_invocation::ToolImageBlock;
use loopal_tool_read_image::{ReadImageParams, ReadImageTool};
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, ToolResult as TurnToolResult, Turn, TurnBody, TurnStep, TurnTrigger,
};
use serde_json::json;

pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

pub fn minimal_png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

pub fn padded_png(width: u32, height: u32, total_bytes: usize) -> Vec<u8> {
    let mut bytes = minimal_png(width, height);
    bytes.resize(total_bytes, 0);
    bytes
}

pub fn backend_for(cwd: &Path) -> Arc<LocalBackend> {
    LocalBackend::new(
        cwd.to_path_buf(),
        None,
        ResourceLimits::default(),
        "e2e-session",
    )
}

pub async fn read_image(cwd: &Path, file: &Path) -> ToolResult {
    let tool: TypedBridge<ReadImageTool, ReadImageParams> = TypedBridge::new(ReadImageTool);
    let context = ToolContext::new(backend_for(cwd), "e2e-session");
    tool.execute(json!({"file_path": file.to_str().unwrap()}), &context)
        .await
        .unwrap()
}

pub async fn persist_images(
    store: &dyn ResourceStore,
    images: Vec<ToolImageBlock>,
) -> Vec<ToolImageBlock> {
    let mut images = images;
    maybe_persist_inline_images(
        store,
        "e2e-session",
        &mut images,
        256 * 1024,
        MAX_IMAGE_BYTES,
    )
    .await
    .unwrap();
    images
}

pub fn anthropic_params(turns: Vec<Turn>) -> ChatParams {
    ChatParams {
        model: "claude-test".into(),
        turns,
        system_prompt: String::new(),
        tools: Vec::<ToolDefinition>::new(),
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

pub fn turn_with_tool_images(tool_use_id: &str, images: Vec<ToolImageBlock>) -> Turn {
    let call = ToolCall {
        id: ToolCallId::new(tool_use_id),
        name: "Read".into(),
        input: json!({}),
    };
    let mut turn = Turn::new(TurnTrigger::UserInput {
        envelope_id: "env".into(),
        content: "show me the image".into(),
        images: Vec::new(),
    });
    turn.body = TurnBody {
        steps: vec![
            TurnStep::LlmCall {
                model: "claude-test".into(),
                response: AssistantOutput {
                    text_blocks: vec![],
                    tool_calls: vec![call.clone()],
                    server_blocks: vec![],
                    stop_reason: StopReason::ToolUse,
                },
            },
            TurnStep::ToolBatch(OrderedToolBatch {
                items: vec![ToolBatchItem {
                    call,
                    state: ToolExecState::Done(TurnToolResult {
                        content: String::new(),
                        is_error: false,
                        images,
                        metadata: None,
                    }),
                }],
            }),
        ],
    };
    turn
}
