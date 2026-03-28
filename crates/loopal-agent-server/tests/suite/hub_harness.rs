//! Shared harness: HubFrontend + real agent loop for full-stack interaction tests.
//!
//! Mirrors the production path in `session_start.rs` but uses in-process channels
//! and mock provider instead of real IPC transport + LLM.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc, watch};

use loopal_error::AgentOutput;
use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, InterruptSignal, MessageSource};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;
use loopal_test_support::scenarios::Calls;

use loopal_agent_server::testing::{
    InputFromClient, SharedSession, StartParams, build_kernel_with_provider,
};

pub const T: Duration = Duration::from_secs(10);

pub struct HubTestHarness {
    pub input_tx: mpsc::Sender<InputFromClient>,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<watch::Sender<u64>>,
    pub client_rx: mpsc::Receiver<Incoming>,
    pub client_conn: Arc<Connection>,
    #[allow(dead_code)]
    pub agent_task: tokio::task::JoinHandle<loopal_error::Result<AgentOutput>>,
    #[allow(dead_code)]
    pub fixture: TestFixture,
}

impl HubTestHarness {
    /// Wait for the agent loop to be ready (consume startup events up to AwaitingInput).
    pub async fn wait_ready(&mut self) {
        let events = self.collect_events().await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEventPayload::AwaitingInput)),
            "agent should reach AwaitingInput on startup"
        );
    }

    /// Send a user message to the agent loop.
    pub async fn send_message(&self, text: &str) {
        let env = Envelope::new(MessageSource::Human, "main", text);
        self.input_tx
            .send(InputFromClient::Message(env))
            .await
            .expect("input channel open");
    }

    /// Signal interrupt (same as TUI interrupt path).
    pub fn interrupt(&self) {
        self.interrupt.signal();
        self.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
    }

    /// Collect agent events until AwaitingInput or Finished.
    pub async fn collect_events(&mut self) -> Vec<AgentEventPayload> {
        collect_agent_events(&mut self.client_rx).await
    }
}

fn conn_pair() -> (Arc<Connection>, Arc<Connection>, mpsc::Receiver<Incoming>) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let server_t: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    let client_t: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let server_conn = Arc::new(Connection::new(server_t));
    let _server_rx = server_conn.start();
    let client_conn = Arc::new(Connection::new(client_t));
    let client_rx = client_conn.start();
    (server_conn, client_conn, client_rx)
}

pub async fn build_hub_harness(calls: Calls) -> HubTestHarness {
    let mut h = build_hub_harness_with(calls, true, None).await;
    h.wait_ready().await;
    h
}

pub async fn build_hub_harness_with(
    calls: Calls,
    interactive: bool,
    permission_mode: Option<loopal_tool_api::PermissionMode>,
) -> HubTestHarness {
    let fixture = TestFixture::new();
    let provider = Arc::new(MultiCallProvider::new(calls));
    let kernel = build_kernel_with_provider(provider).unwrap();

    let (input_tx, input_rx) = mpsc::channel::<InputFromClient>(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, watch_rx) = watch::channel(0u64);
    let interrupt_tx = Arc::new(watch_tx);

    let session = Arc::new(SharedSession {
        session_id: "hub-test".into(),
        clients: Mutex::new(Vec::new()),
        input_tx: input_tx.clone(),
        interrupt: interrupt.clone(),
        interrupt_tx: interrupt_tx.clone(),
    });
    let (server_conn, client_conn, client_rx) = conn_pair();
    session.add_client("test".into(), server_conn).await;

    let frontend = loopal_agent_server::hub_frontend_for_test(session, input_rx, watch_rx);
    let mut config = loopal_config::load_config(fixture.path()).unwrap();
    if let Some(pm) = permission_mode {
        config.settings.permission_mode = pm;
    }
    let start = StartParams {
        cwd: None,
        model: None,
        mode: None,
        prompt: None,
        permission_mode: None,
        no_sandbox: true,
        resume: None,
    };
    // Mock hub connection for tests (in-memory duplex).
    let (hub_conn, _hub_peer) = loopal_ipc::duplex_pair();
    let hub_connection = std::sync::Arc::new(loopal_ipc::Connection::new(hub_conn));

    let params = loopal_agent_server::testing::build_with_frontend(
        fixture.path(),
        &config,
        &start,
        frontend,
        interrupt.clone(),
        interrupt_tx.clone(),
        kernel,
        hub_connection,
        Some(fixture.path()),
        interactive,
    )
    .unwrap();

    let agent_task = tokio::spawn(loopal_runtime::agent_loop(params));

    HubTestHarness {
        input_tx,
        interrupt,
        interrupt_tx,
        client_rx,
        client_conn,
        agent_task,
        fixture,
    }
}

/// Collect agent/event notifications until Finished or AwaitingInput.
pub async fn collect_agent_events(rx: &mut mpsc::Receiver<Incoming>) -> Vec<AgentEventPayload> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<AgentEvent>(params) {
                        let terminal = matches!(
                            ev.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        );
                        events.push(ev.payload);
                        if terminal {
                            break;
                        }
                    }
                }
            }
            Ok(Some(Incoming::Request { .. })) => {
                // Permission/question requests handled by specific tests
                break;
            }
            _ => break,
        }
    }
    events
}

/// Check if events contain a Stream event with the given substring.
pub fn has_stream(events: &[AgentEventPayload], needle: &str) -> bool {
    events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Stream { text } if text.contains(needle)))
}
