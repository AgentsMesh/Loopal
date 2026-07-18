use std::time::Duration;

use serde_json::json;

use crate::support::HubHarness;

/// The Hub-owned MCP path the serve-mode suite cannot reach: the real Hub
/// spawns the real `mock_mcp_server` subprocess from settings, the root agent
/// registers its tool through the Hub proxy, and a model tool_use round-trips
/// LLM wire → agent → Hub → MCP subprocess → back.
#[tokio::test]
async fn hub_spawned_mcp_tool_round_trips_through_a_real_turn() {
    let mut h = HubHarness::start_with_mcp(json!({
        "version": 2,
        "name": "hub_mcp",
        "calls": [
            {"expect": {"userContains": "echo through the hub"},
             "chunks": [
                {"type": "tool_use", "id": "hm1", "name": "mcp_echo",
                 "input": {"text": "real-hub-wire"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "hm1"},
             "chunks": [{"type": "text", "text": "hub mcp worked"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    // The Hub spawns MCP in the background (non-blocking startup contract);
    // give the spawn + dynamic tool registration a moment to settle.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let out = h.turn("please echo through the hub").await;
    assert!(
        out.error.is_none() && out.finished,
        "turn failed: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.text.contains("hub mcp worked"),
        "the continuation only returns this text if the MCP round-trip \
         succeeded; text: {:?}\nevents: {:?}",
        out.text,
        out.events
    );
    assert!(
        out.events.iter().any(|e| e.starts_with("ToolResult")
            && e.contains("mcp_echo: real-hub-wire")
            && e.contains("is_error: false")),
        "the real MCP subprocess's output must come back through the Hub; \
         events: {:?}",
        out.events
    );
}
