use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::{McpServerConfig, McpSharing, Settings};
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEventPayload, ControlCommand};
use loopal_runtime::agent_input::{AgentInput, ControlAcknowledgement, ControlRequest};

use super::{make_runner_with_channels, make_runner_with_tracked_kernel};

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
async fn healthy_local_reconnect_registers_tools_and_is_applied() {
    let url = super::input_mcp_http_fixture::spawn().await;
    let mut settings = Settings::default();
    settings.mcp_servers.insert(
        "healthy".into(),
        McpServerConfig::StreamableHttp {
            url,
            headers: HashMap::new(),
            enabled: true,
            timeout_ms: 2_000,
            sharing: McpSharing::HubSingleton,
        },
    );
    let kernel = Arc::new(Kernel::new(settings).unwrap());
    kernel.spawn_mcp().await;
    assert!(
        kernel
            .local_mcp_provider()
            .unwrap()
            .wait_until_settled(Duration::from_secs(2))
            .await
    );
    assert!(kernel.get_tool("runtime_recovered_tool").is_none());
    let (mut runner, mut events, inputs) = make_runner_with_tracked_kernel(kernel.clone());
    let (request, mut acknowledgement) = ControlRequest::tracked(ControlCommand::McpReconnect {
        server: "healthy".into(),
    });
    inputs
        .send(AgentInput::TrackedControl(request))
        .await
        .unwrap();

    let _ = tokio::time::timeout(Duration::from_millis(200), runner.wait_for_input()).await;

    assert_eq!(
        acknowledgement.recv().await,
        Some(ControlAcknowledgement::Applied)
    );
    assert!(kernel.get_tool("runtime_recovered_tool").is_some());
    assert!(drain_for_status(&mut events).await);
}

#[tokio::test]
async fn unknown_proxy_reconnect_is_rejected() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let provider = kernel.mcp_provider();
    kernel.set_mcp_provider(provider);
    let (mut runner, mut events, inputs) = make_runner_with_tracked_kernel(Arc::new(kernel));
    let (request, mut acknowledgement) = ControlRequest::tracked(ControlCommand::McpReconnect {
        server: "proxied".into(),
    });
    inputs
        .send(AgentInput::TrackedControl(request))
        .await
        .unwrap();

    let _ = tokio::time::timeout(Duration::from_millis(200), runner.wait_for_input()).await;

    assert_eq!(
        acknowledgement.recv().await,
        Some(ControlAcknowledgement::Rejected(
            "MCP reconnect failed for proxied".into()
        ))
    );
    assert!(drain_for_status(&mut events).await);
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
