//! Shared test helpers for bridge_basic_test and bridge_edge_test.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;

pub const T: Duration = Duration::from_secs(10);

pub fn make_duplex_pair() -> (Arc<dyn Transport>, Arc<dyn Transport>) {
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
    (server_t, client_t)
}

/// Start a mock sub-agent server. Returns client connection + incoming receiver + join handle.
pub async fn start_child_server(
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (
    Arc<Connection>,
    tokio::sync::mpsc::Receiver<Incoming>,
    TestFixture,
    tokio::task::JoinHandle<()>,
) {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let (server_t, client_t) = make_duplex_pair();
    let join = tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });
    let conn = Arc::new(Connection::new(client_t));
    let rx = conn.start();
    (conn, rx, fixture, join)
}

/// Initialize + start agent with prompt, return session_id.
pub async fn init_and_start(conn: &Connection, fixture: &TestFixture, prompt: &str) -> String {
    init_and_start_with(conn, fixture, prompt, serde_json::json!({})).await
}

/// Initialize + start agent with prompt and extra params, return session_id.
pub async fn init_and_start_with(
    conn: &Connection,
    fixture: &TestFixture,
    prompt: &str,
    extra: serde_json::Value,
) -> String {
    tokio::time::timeout(
        T,
        conn.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();
    let mut params = serde_json::json!({
        "prompt": prompt,
        "cwd": fixture.path().to_string_lossy().as_ref(),
    });
    if let serde_json::Value::Object(map) = extra {
        for (k, v) in map {
            params[k] = v;
        }
    }
    let resp = tokio::time::timeout(T, conn.send_request(methods::AGENT_START.name, params))
        .await
        .unwrap()
        .unwrap();
    resp["session_id"].as_str().unwrap().to_string()
}

/// Collect agent/event notifications until Finished or AwaitingInput.
pub async fn collect_agent_events(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
) -> Vec<AgentEventPayload> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name
                    && let Ok(ev) = serde_json::from_value::<AgentEvent>(params)
                {
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
            _ => break,
        }
    }
    events
}
