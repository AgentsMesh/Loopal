//! E2E MCP-style tool tests: custom tool via kernel_setup, unknown external tool.

use async_trait::async_trait;
use serde_json::Value;

use loopal_error::LoopalError;
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, assertions, chunks};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

// ── Mock external tool ──────────────────────────────────────────────

struct MockExternalTool;

#[async_trait]
impl Tool for MockExternalTool {
    fn name(&self) -> &str {
        "MockExternal"
    }
    fn description(&self) -> &str {
        "A mock external/MCP-style tool"
    }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            }
        })
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let query = input["query"].as_str().unwrap_or("none");
        Ok(ToolResult::success(format!(
            "MockExternal result for: {query}"
        )))
    }
}

/// Tool that always returns Err (simulates runtime failure in MCP tool).
struct FailingExternalTool;

#[async_trait]
impl Tool for FailingExternalTool {
    fn name(&self) -> &str {
        "FailingExternal"
    }
    fn description(&self) -> &str {
        "Always fails with an error"
    }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({"type": "object", "properties": {}})
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    async fn execute(&self, _: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        Err(LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
            "simulated MCP transport failure".into(),
        )))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn wrap_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        Vec::<CommandEntry>::new(),
        inner.fixture.path().to_path_buf(),
    );
    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_mcp_style_tool_execution() {
    let calls = vec![
        chunks::tool_turn(
            "tc-ext",
            "MockExternal",
            serde_json::json!({"query": "test_query"}),
        ),
        chunks::text_turn("External tool executed."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .kernel_setup(|kernel| {
            kernel.register_tool(Box::new(MockExternalTool));
        })
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "MockExternal");
    assertions::assert_has_tool_result(&evts, "MockExternal", false);

    // Verify the tool result contains expected output
    let results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "MockExternal" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        results.iter().any(|r| r.contains("test_query")),
        "tool result should contain 'test_query', got: {results:?}"
    );
}

#[tokio::test]
async fn test_unknown_external_tool() {
    // Provider calls a tool name that's not registered anywhere
    let calls = vec![
        chunks::tool_turn("tc-unk", "UnregisteredMcpTool", serde_json::json!({})),
        chunks::text_turn("Unknown tool handled."),
    ];
    let mut harness = super::e2e_harness::build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    // Unknown tool → error result
    assertions::assert_has_tool_result(&evts, "UnregisteredMcpTool", true);
    assertions::assert_has_stream(&evts);
}

#[tokio::test]
async fn test_mcp_tool_execution_error() {
    // A registered tool that returns Err(LoopalError) — runtime wraps it as error ToolResult
    let calls = vec![
        chunks::tool_turn("tc-fail", "FailingExternal", serde_json::json!({})),
        chunks::text_turn("Failure handled."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .kernel_setup(|kernel| {
            kernel.register_tool(Box::new(FailingExternalTool));
        })
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "FailingExternal");
    assertions::assert_has_tool_result(&evts, "FailingExternal", true);

    // Verify error message contains the failure description
    let err_results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "FailingExternal" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        err_results
            .iter()
            .any(|r| r.contains("simulated MCP transport failure")),
        "error should contain failure message, got: {err_results:?}"
    );
    assertions::assert_has_stream(&evts);
}
