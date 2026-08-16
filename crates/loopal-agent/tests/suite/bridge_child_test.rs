//! Tests for the legacy direct-client bridge's authoritative completion contract.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use loopal_agent::bridge::bridge_child_events;
use loopal_agent_client::AgentClient;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, AgentEvent, AgentEventPayload};
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;
use loopal_test_support::mock_provider::MultiCallProvider;
use loopal_test_support::scenarios;

pub(crate) use loopal_test_support::make_duplex_pair;

const T: Duration = Duration::from_secs(10);

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
            sandbox_policy: None,
            session_id: None,
            ..Default::default()
        })
        .await
        .expect("start_agent");

    let (event_tx, _event_rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    (client, event_tx, cancel, fixture)
}

// ── Tests ────────────────────────────────────────────────────────────

/// A real child result crosses server IPC through the authoritative completion.
#[tokio::test]
async fn bridge_returns_completed_result() {
    let (client, event_tx, cancel, _fix) =
        start_bridge_client(scenarios::simple_text("hello from sub-agent")).await;

    let result = tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel))
        .await
        .unwrap();

    let text = result.expect("should succeed");
    assert!(
        text.contains("hello from sub-agent"),
        "should return completed result, got: {text}"
    );
}

/// Stream and Finished are observational; only agent/completed supplies result.
#[tokio::test]
async fn bridge_ignores_stream_as_result_and_waits_for_completion() {
    let (server_transport, client_transport) = make_duplex_pair();
    let (server, _server_rx) = Connection::new(server_transport).into_listening();
    let client = AgentClient::new(client_transport);

    for payload in [
        AgentEventPayload::Stream {
            text: "observational stream text".into(),
        },
        AgentEventPayload::Finished,
    ] {
        server
            .send_notification(
                methods::AGENT_EVENT.name,
                serde_json::to_value(AgentEvent::root(payload)).unwrap(),
            )
            .await
            .unwrap();
    }
    server
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::to_value(AgentCompletion {
                reason: "goal".into(),
                result: Some("authoritative result".into()),
            })
            .unwrap(),
        )
        .await
        .unwrap();

    let (event_tx, _event_rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let result = tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel))
        .await
        .unwrap();

    assert_eq!(result.unwrap(), "authoritative result");
}

/// A real child sends agent/completed after Finished and the bridge exits cleanly.
#[tokio::test]
async fn bridge_exits_on_agent_completed() {
    let (client, event_tx, cancel, _fix) =
        start_bridge_client(scenarios::simple_text("done")).await;

    let result = tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel))
        .await
        .expect("bridge should exit on agent/completed, not hang");

    assert_eq!(result.unwrap(), "done");
}

/// Cancel token fired -> bridge sends shutdown and reports cancellation.
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
            sandbox_policy: None,
            session_id: None,
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

    let result = result.expect("bridge should exit after cancel");
    assert_eq!(result.unwrap_err(), "sub-agent test cancelled");
}
