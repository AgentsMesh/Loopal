use loopal_protocol::{ControlCommand, ControlDisposition};
use serde_json::json;

use crate::support::{GatedMcpServer, HubEnv, HubHarness};

#[tokio::test]
async fn failed_hub_owned_mcp_reconnects_and_serves_the_root_agent() {
    let server = GatedMcpServer::start().await;
    let env = HubEnv::new();
    write_settings(env.home.path(), &server.url);
    let mut harness = HubHarness::launch(
        env,
        json!({
            "version": 2,
            "name": "hub_mcp_reconnect",
            "calls": [
                {"expect": {"userContains": "use the restored MCP tool"},
                 "chunks": [
                    {"type": "tool_use", "id": "reconnect-1", "name": "mcp_echo",
                     "input": {"text": "after-reconnect"}},
                    {"type": "done"}
                 ]},
                {"expect": {"toolResultId": "reconnect-1"},
                 "chunks": [
                    {"type": "text", "text": "restored MCP worked"},
                    {"type": "done"}
                 ]}
            ],
            "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
        }),
        false,
    )
    .await;

    assert_eq!(
        harness.control(ControlCommand::QueryMcpStatus).await,
        ControlDisposition::Applied
    );
    let failed = harness.await_mcp_server("recovering").await;
    assert!(failed.status.starts_with("failed:"), "{failed:?}");
    assert_eq!(failed.tool_count, 0);

    server.enable();
    assert_eq!(
        harness
            .control(ControlCommand::McpReconnect {
                server: "recovering".into(),
            })
            .await,
        ControlDisposition::Applied
    );
    let connected = harness.await_mcp_server("recovering").await;
    assert_eq!(connected.status, "connected");
    assert_eq!(connected.tool_count, 1);

    let outcome = harness.turn("please use the restored MCP tool").await;
    assert!(outcome.error.is_none() && outcome.finished, "{outcome:?}");
    assert!(outcome.text.contains("restored MCP worked"), "{outcome:?}");
    assert!(
        outcome
            .events
            .iter()
            .any(|event| event.starts_with("ToolResult")
                && event.contains("mcp_echo: after-reconnect")
                && event.contains("is_error: false")),
        "{:?}",
        outcome.events
    );
    let journal = harness.journal().await;
    assert!(
        journal[0]["toolNames"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|tool| tool == "mcp_echo")),
        "{journal}"
    );
}

fn write_settings(home: &std::path::Path, url: &str) {
    let directory = home.join(".loopal");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("settings.json"),
        serde_json::to_vec_pretty(&json!({
            "mcp_servers": {"recovering": {
                "type": "streamable-http",
                "url": url,
                "enabled": true,
                "timeout_ms": 500,
                "sharing": "hub-singleton"
            }}
        }))
        .unwrap(),
    )
    .unwrap();
}
