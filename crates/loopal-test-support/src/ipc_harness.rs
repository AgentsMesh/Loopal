//! IPC acceptance test harness — full-stack IPC testing with mock provider.
//!
//! Wires up: AgentClient → IPC → Server(IpcFrontend + agent_loop) → IPC → Bridge → channels.
//! Uses in-memory duplex streams from `loopal_ipc::duplex_pair` (no real subprocess).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use loopal_error::LoopalError;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, Envelope, UserQuestionResponse,
};
use loopal_provider_api::StreamChunk;

use crate::fixture::TestFixture;
use crate::make_duplex_pair;
use crate::mock_provider::MultiCallProvider;

/// Full-stack IPC test harness with TUI-side channel handles.
pub struct IpcTestHarness {
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub mailbox_tx: mpsc::Sender<Envelope>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub permission_tx: mpsc::Sender<bool>,
    pub question_tx: mpsc::Sender<UserQuestionResponse>,
    pub fixture: TestFixture,
}

/// Build a full-stack IPC harness with mock provider.
///
/// The server runs in a background task with IpcFrontend + agent_loop.
/// The client side uses AgentClient → into_parts → bridge.
/// Returns TUI-side channel handles for sending messages and collecting events.
pub async fn build_ipc_harness(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
) -> IpcTestHarness {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");

    // In-memory duplex transport pair — single source of truth for
    // duplex transports across the test toolbox.
    let (server_transport, client_transport) = make_duplex_pair();

    // Spawn server in background
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_transport, provider, cwd, session_dir)
                .await;
    });

    // Client handshake — send a dummy prompt so agent starts processing
    let client = loopal_agent_client::AgentClient::new(client_transport);
    client.initialize().await.expect("IPC initialize failed");
    client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: fixture.path().to_path_buf(),
            prompt: Some("hello".to_string()),
            ..Default::default()
        })
        .await
        .expect("agent/start failed");

    // Hand connection to bridge
    let (connection, incoming_rx) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(connection, incoming_rx);

    IpcTestHarness {
        event_rx: handles.agent_event_rx,
        mailbox_tx: handles.mailbox_tx,
        control_tx: handles.control_tx,
        permission_tx: handles.permission_tx,
        question_tx: handles.question_tx,
        fixture,
    }
}

/// Default timeout for acceptance tests.
pub const IPC_TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Collect events until AwaitingInput or Finished, with timeout.
pub async fn collect_ipc_events(rx: &mut mpsc::Receiver<AgentEvent>) -> Vec<AgentEventPayload> {
    let mut events = Vec::new();
    loop {
        match tokio::time::timeout(IPC_TEST_TIMEOUT, rx.recv()).await {
            Ok(Some(event)) => {
                let is_terminal = matches!(
                    &event.payload,
                    AgentEventPayload::AwaitingInput | AgentEventPayload::Finished
                );
                events.push(event.payload);
                if is_terminal {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => {
                panic!(
                    "IPC test timeout after {:?}. Collected {} events so far.",
                    IPC_TEST_TIMEOUT,
                    events.len()
                );
            }
        }
    }
    events
}
