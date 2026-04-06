//! Tests for bridge_child_events: the core logic that collects sub-agent output.
//! Verifies what the parent Agent tool actually receives as the sub-agent's result.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use loopal_agent::bridge::bridge_child_events;
use loopal_agent_client::AgentClient;
use loopal_ipc::StdioTransport;
use loopal_ipc::transport::Transport;
use loopal_protocol::AgentEvent;
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;
use loopal_test_support::mock_provider::MultiCallProvider;
use loopal_test_support::scenarios;

const T: Duration = Duration::from_secs(10);

pub(crate) fn make_duplex_pair() -> (Arc<dyn Transport>, Arc<dyn Transport>) {
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

/// Start a mock child server, return an initialized+started AgentClient.
pub(crate) async fn start_bridge_client(
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (
    AgentClient,
    mpsc::Sender<AgentEvent>,
    CancellationToken,
    TestFixture,
) {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let (server_t, client_t) = make_duplex_pair();

    tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });

    let client = AgentClient::new(client_t);
    client.initialize().await.expect("initialize");
    client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: fixture.path().to_path_buf(),
            prompt: Some("work".to_string()),
            ..Default::default()
        })
        .await
        .expect("start_agent");

    let (event_tx, _event_rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    (client, event_tx, cancel, fixture)
}

// ── Tests ────────────────────────────────────────────────────────────

/// Sub-agent streams text -> bridge returns accumulated stream text.
#[tokio::test]
async fn bridge_returns_stream_text() {
    let (client, event_tx, cancel, _fix) =
        start_bridge_client(scenarios::simple_text("hello from sub-agent")).await;

    let result = tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel))
        .await
        .unwrap();

    let text = result.expect("should succeed");
    assert!(
        text.contains("hello from sub-agent"),
        "should return stream text, got: {text}"
    );
}

/// Sub-agent produces no text (tools only, then finish) -> default message.
#[tokio::test]
async fn bridge_returns_default_when_no_output() {
    let calls = vec![chunks::tool_turn(
        "tc-1",
        "Ls",
        serde_json::json!({"path": "."}),
    )];
    let (client, event_tx, cancel, _fix) = start_bridge_client(calls).await;

    let result = tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel))
        .await
        .unwrap();

    let text = result.expect("should succeed");
    assert!(!text.is_empty(), "should return non-empty text");
}

/// Finished event -> bridge exits cleanly within timeout (regression for hang bug).
#[tokio::test]
async fn bridge_exits_on_finished() {
    let (client, event_tx, cancel, _fix) =
        start_bridge_client(scenarios::simple_text("done")).await;

    let result =
        tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel)).await;

    assert!(result.is_ok(), "bridge should exit on Finished, not hang");
}

/// Cancel token fired -> bridge sends shutdown and exits.
#[tokio::test]
async fn bridge_cancel_sends_shutdown() {
    let calls = vec![vec![
        chunks::text("slow..."),
        chunks::usage(5, 3),
        chunks::done(),
    ]];
    let provider = Arc::new(MultiCallProvider::new(calls).with_delay(Duration::from_secs(5)))
        as Arc<dyn loopal_provider_api::Provider>;

    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let (server_t, client_t) = make_duplex_pair();
    tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });

    let client = AgentClient::new(client_t);
    client.initialize().await.unwrap();
    client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: fixture.path().to_path_buf(),
            prompt: Some("slow task".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();

    let (event_tx, _rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel2.cancel();
    });

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        bridge_child_events(client, &event_tx, "test", &cancel),
    )
    .await;

    assert!(result.is_ok(), "bridge should exit after cancel");
}
