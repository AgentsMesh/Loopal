use serde_json::json;

use crate::support::CliHarness;

/// An MCP tool exercised through a real turn. The cwd config declares an MCP
/// server; the serve-mode agent proxies MCP over IPC to its Hub peer (played
/// by the harness), registers the advertised `mcp_echo` tool during
/// `agent/start`, and the model's tool_use round-trips: LLM wire → kernel →
/// MCP proxy → Hub → tool result → LLM wire → final text.
#[tokio::test]
async fn agent_calls_an_mcp_tool_through_the_hub_proxy() {
    let mut h = CliHarness::start_with_mcp(json!({
        "version": 2,
        "name": "mcp_tool",
        "calls": [
            {"expect": {"userContains": "echo over mcp"},
             "chunks": [
                {"type": "tool_use", "id": "m1", "name": "mcp_echo",
                 "input": {"text": "over-the-wire"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "m1"},
             "chunks": [{"type": "text", "text": "mcp tool worked"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("please echo over mcp").await;
    assert!(
        out.error.is_none(),
        "turn errored: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.finished,
        "turn did not finish; events: {:?}",
        out.events
    );
    assert!(
        out.text.contains("mcp tool worked"),
        "the follow-up call only returns this text if the MCP tool round-trip \
         succeeded; text: {:?}\nevents: {:?}",
        out.text,
        out.events
    );
    assert!(
        out.events.iter().any(|e| e.starts_with("ToolResult")
            && e.contains("mcp_echo: over-the-wire")
            && e.contains("is_error: false")),
        "expected a successful mcp_echo ToolResult event; events: {:?}",
        out.events
    );

    let calls = h.mcp_calls();
    assert_eq!(
        calls.len(),
        1,
        "exactly one hub/mcp/call_tool must reach the Hub peer; got: {calls:?}"
    );
    assert_eq!(calls[0]["server"], "mock");
    assert_eq!(calls[0]["tool"], "mcp_echo");
    assert_eq!(calls[0]["args"]["text"], "over-the-wire");
}
