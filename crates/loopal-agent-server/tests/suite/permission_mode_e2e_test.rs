//! E2E tests for permission_mode / decision_mode independent override via IPC.
//!
//! These tests verify that the full IPC path (client → server → session_start →
//! apply_start_overrides) correctly handles independent config dimensions.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;
use loopal_test_support::mock_provider::MultiCallProvider;

async fn start_test_server(
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (
    Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
    TestFixture,
) {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;

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
        let _ =
            loopal_agent_server::run_server_for_test(server_transport, provider, cwd, session_dir)
                .await;
    });

    let (client, rx) = Connection::new(client_transport).into_listening();
    (client, rx, fixture)
}

async fn init_and_start(
    client: &Arc<Connection<Listening>>,
    extra_params: serde_json::Value,
) -> serde_json::Value {
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();

    let mut params = serde_json::json!({"prompt": "test"});
    if let serde_json::Value::Object(map) = extra_params {
        for (k, v) in map {
            params[k] = v;
        }
    }

    tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request(methods::AGENT_START.name, params),
    )
    .await
    .unwrap()
    .unwrap()
}

async fn collect_events_until_terminal(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    conn: &Arc<Connection<Listening>>,
) -> Vec<AgentEventPayload> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
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
            Ok(Some(Incoming::Request { id, method, .. })) => {
                // Auto-deny permission requests for this test
                if method == methods::AGENT_PERMISSION.name {
                    // This should NOT happen when permission_mode is Bypass
                    events.push(AgentEventPayload::Error {
                        message: "UNEXPECTED_PERMISSION_REQUEST".into(),
                    });
                }
                let _ = conn.respond(id, serde_json::json!({"allow": false})).await;
            }
            _ => break,
        }
    }
    events
}

/// Regression test: specifying only decision_mode should NOT change permission_mode.
///
/// This is the exact bug scenario: user runs `loopal --decision classifier` expecting
/// Bypass + Classifier, but the old code produced AskAnyWrite + Classifier, causing
/// tools to be blocked by the classifier.
#[tokio::test]
async fn only_decision_mode_keeps_permission_bypass() {
    // Simple tool call that would trigger permission check if mode != Bypass
    let calls = vec![
        chunks::tool_turn(
            "tc-read",
            "Read",
            serde_json::json!({"file_path": "/tmp/test.txt"}),
        ),
        chunks::text_turn("done"),
    ];
    let (client, mut rx, _fixture) = start_test_server(calls).await;

    // Start with ONLY decision_mode specified — permission_mode should remain Bypass
    let resp = init_and_start(&client, serde_json::json!({"decision_mode": "classifier"})).await;
    assert!(resp.get("session_id").is_some());

    let events = collect_events_until_terminal(&mut rx, &client).await;

    // If permission_mode incorrectly became AskAnyWrite, we'd see a permission request
    // (which our collector marks as UNEXPECTED_PERMISSION_REQUEST error)
    let unexpected_perm = events.iter().any(|e| {
        matches!(e, AgentEventPayload::Error { message } if message.contains("UNEXPECTED_PERMISSION_REQUEST"))
    });
    assert!(
        !unexpected_perm,
        "permission_mode should remain Bypass when only decision_mode is specified — \
         no permission request should occur for ReadOnly tools"
    );

    // Should complete normally
    let finished = events.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
        )
    });
    assert!(finished, "session should complete normally");
}

/// Verify that specifying only permission_mode does NOT change decision_mode.
#[tokio::test]
async fn only_permission_mode_keeps_decision_manual() {
    let calls = vec![chunks::text_turn("hello")];
    let (client, mut rx, _fixture) = start_test_server(calls).await;

    // Start with ONLY permission_mode specified
    let resp = init_and_start(
        &client,
        serde_json::json!({"permission_mode": "ask_dangerous"}),
    )
    .await;
    assert!(resp.get("session_id").is_some());

    let events = collect_events_until_terminal(&mut rx, &client).await;

    // Should complete normally (no classifier involved, just manual mode)
    let finished = events.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
        )
    });
    assert!(
        finished,
        "session should complete normally with manual decision mode"
    );
}

/// Verify that specifying neither keeps both at defaults (Bypass + Manual).
#[tokio::test]
async fn neither_specified_uses_defaults() {
    let calls = vec![chunks::text_turn("hello")];
    let (client, mut rx, _fixture) = start_test_server(calls).await;

    // Start with no permission/decision params
    let resp = init_and_start(&client, serde_json::json!({})).await;
    assert!(resp.get("session_id").is_some());

    let events = collect_events_until_terminal(&mut rx, &client).await;

    // Should complete normally with defaults
    let finished = events.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
        )
    });
    assert!(
        finished,
        "session should complete normally with default settings"
    );
}
