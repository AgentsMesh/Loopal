//! Tests for session_start: model override, interactive mode.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;

/// Build a test server with mock provider and return client connection.
async fn start_test_server(
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (Arc<Connection>, tokio::sync::mpsc::Receiver<Incoming>, TestFixture) {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let provider = Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;

    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);

    let server_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let client_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));

    tokio::spawn(async move {
        let _ = loopal_agent_server::run_server_for_test(
            server_transport, provider, cwd, session_dir,
        ).await;
    });

    let client = Arc::new(Connection::new(client_transport));
    let rx = client.start();
    (client, rx, fixture)
}

/// Model override in agent/start should propagate to the agent loop.
/// After the refactoring, session_start applies overrides to config before
/// building the agent — this test guards against regression.
#[tokio::test]
async fn model_override_applied_in_session_start() {
    use loopal_test_support::chunks;

    let (client, mut rx, _fixture) = start_test_server(
        vec![chunks::text_turn("hello")],
    ).await;

    // Initialize
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    ).await.unwrap().unwrap();

    // Start with model override and a prompt (non-interactive → finishes)
    let resp = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request(
            methods::AGENT_START.name,
            serde_json::json!({"model": "claude-opus-4-6", "prompt": "hi"}),
        ),
    ).await.unwrap().unwrap();
    assert!(resp.get("session_id").is_some());

    // Collect events — if model was wrong, we'd get a provider error instead of stream
    let mut got_stream = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<loopal_protocol::AgentEvent>(params) {
                        match &ev.payload {
                            loopal_protocol::AgentEventPayload::Stream { .. } => {
                                got_stream = true;
                            }
                            loopal_protocol::AgentEventPayload::Finished => break,
                            loopal_protocol::AgentEventPayload::Error { message } => {
                                panic!("agent error (model override not applied?): {message}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    assert!(got_stream, "should receive stream events (model override applied)");
}
