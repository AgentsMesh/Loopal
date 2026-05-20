use std::time::Duration;

use loopal_protocol::{AgentEventPayload, ControlCommand};

use super::make_runner_with_channels;

async fn drain_for_status(
    event_rx: &mut tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
) -> bool {
    while let Ok(event) = tokio::time::timeout(Duration::from_millis(50), event_rx.recv()).await {
        let Some(event) = event else { break };
        if matches!(event.payload, AgentEventPayload::McpStatusReport { .. }) {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn query_mcp_status_emits_snapshot_via_provider() {
    let (mut runner, mut event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    ctrl_tx.send(ControlCommand::QueryMcpStatus).await.unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(200), runner.wait_for_input()).await;

    let mut saw_empty_report = false;
    while let Ok(Some(event)) =
        tokio::time::timeout(Duration::from_millis(50), event_rx.recv()).await
    {
        if let AgentEventPayload::McpStatusReport { servers } = event.payload {
            assert!(
                servers.is_empty(),
                "default settings → empty MCP server list"
            );
            saw_empty_report = true;
            break;
        }
    }
    assert!(saw_empty_report, "QueryMcpStatus must emit McpStatusReport");
}

#[tokio::test]
async fn mcp_reconnect_on_unknown_server_still_emits_report() {
    let (mut runner, mut event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    ctrl_tx
        .send(ControlCommand::McpReconnect {
            server: "phantom-server".into(),
        })
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(200), runner.wait_for_input()).await;

    assert!(
        drain_for_status(&mut event_rx).await,
        "reconnect on unknown server must still emit McpStatusReport (warn, no panic)"
    );
}

#[tokio::test]
async fn mcp_disconnect_on_unknown_server_does_not_panic() {
    let (mut runner, mut event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    ctrl_tx
        .send(ControlCommand::McpDisconnect {
            server: "phantom-server".into(),
        })
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(200), runner.wait_for_input()).await;

    assert!(
        drain_for_status(&mut event_rx).await,
        "disconnect on unknown server must still emit McpStatusReport"
    );
}
