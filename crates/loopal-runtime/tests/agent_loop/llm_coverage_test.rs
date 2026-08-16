use std::io::Write;
use std::process::Command;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_provider_api::{
    ChatParams, ChatStream, Provider, StopReason as ProviderStopReason, StreamChunk,
};
use loopal_storage::{FileResourceStore, ResourceStore};
use loopal_tool_invocation::ToolImageBlock;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason as TurnStopReason, ToolBatchItem, ToolCall,
    ToolCallId, ToolExecState, ToolResult, TurnStep,
};

use super::{in_turn, make_cancel};

const CHILD_ENV: &str = "LOOPAL_LLM_COVERAGE_CHILD";
const TEST_NAME: &str = "agent_loop::llm_coverage_test::stream_llm_hydrates_and_logs";

#[derive(Clone)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

struct HydrationProvider;

#[async_trait]
impl Provider for HydrationProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopalError> {
        let image = params
            .turns
            .iter()
            .flat_map(|turn| &turn.body.steps)
            .find_map(|step| match step {
                TurnStep::ToolBatch(batch) => match &batch.items[0].state {
                    ToolExecState::Done(result) => result.images.first(),
                    _ => None,
                },
                _ => None,
            });
        assert!(matches!(image, Some(ToolImageBlock::Inline { .. })));
        let chunks = vec![
            Ok(StreamChunk::Text { text: "ok".into() }),
            Ok(StreamChunk::ToolUse {
                id: "next".into(),
                name: "Read".into(),
                input: serde_json::json!({}),
            }),
            Ok(StreamChunk::ServerToolUse {
                id: "search".into(),
                name: "web_search".into(),
                input: serde_json::json!({"query": "coverage"}),
            }),
            Ok(StreamChunk::ServerToolResult {
                block_type: "web_search_tool_result".into(),
                tool_use_id: "search".into(),
                content: serde_json::json!({"results": []}),
            }),
            Ok(StreamChunk::Usage {
                input_tokens: 3,
                output_tokens: 2,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                thinking_tokens: 0,
            }),
            Ok(StreamChunk::Done {
                stop_reason: ProviderStopReason::EndTurn,
            }),
        ];
        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}

fn append_resource_result(
    runner: &mut loopal_runtime::agent_loop::AgentLoopRunner,
    id: String,
    byte_size: usize,
) {
    let call = ToolCall {
        id: ToolCallId::new("image"),
        name: "Read".into(),
        input: serde_json::json!({}),
    };
    runner
        .append_step_record(TurnStep::LlmCall {
            model: "claude-test".into(),
            response: AssistantOutput {
                text_blocks: vec![],
                tool_calls: vec![call.clone()],
                server_blocks: vec![],
                stop_reason: TurnStopReason::ToolUse,
            },
        })
        .unwrap();
    runner
        .append_step_record(TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call,
                state: ToolExecState::Done(ToolResult {
                    content: String::new(),
                    is_error: false,
                    metadata: None,
                    images: vec![ToolImageBlock::SessionResource {
                        id,
                        media_type: "image/png".into(),
                        byte_size,
                    }],
                }),
            }],
        }))
        .unwrap();
}

async fn run_child() {
    let logs = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(SharedWriter(logs.clone()))
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let (mut runner, _event_rx) =
        super::mock_provider::make_runner_with_dyn_provider(Arc::new(HydrationProvider));
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let store = FileResourceStore::with_base_dir(home.join(".loopal"));
    runner.params.resource_store = Some(store.clone());
    let bytes = b"\x89PNG\r\n\x1a\n";
    let id = store
        .write(&runner.params.session.id, "image/png", bytes)
        .await
        .unwrap();
    append_resource_result(&mut runner, id, bytes.len());

    let result = in_turn(runner.stream_llm_with(None, &make_cancel()))
        .await
        .unwrap();

    assert_eq!(result.assistant_text, "ok");
    assert_eq!(result.tool_uses.len(), 1);
    assert_eq!(result.server_blocks.len(), 2);

    let (mut rejected, _event_rx) =
        super::mock_provider::make_runner_with_dyn_provider(Arc::new(HydrationProvider));
    rejected.params.resource_store = Some(store);
    append_resource_result(&mut rejected, "invalid".into(), 8);
    in_turn(rejected.stream_llm_with(None, &make_cancel()))
        .await
        .expect_err("invalid resource must fail before provider dispatch");

    let logs = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("LLM request"));
    assert!(logs.contains("LLM complete"));
}

#[test]
fn stream_llm_hydrates_and_logs() {
    if std::env::var_os(CHILD_ENV).is_some() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run_child());
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(CHILD_ENV, "1")
        .env("HOME", home.path())
        .status()
        .unwrap();
    assert!(status.success());
}
