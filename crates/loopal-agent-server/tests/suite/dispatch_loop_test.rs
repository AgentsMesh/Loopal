//! Integration tests for dispatch_loop session cycling and session_forward.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;

async fn start_test_server_with_calls(
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (
    Arc<Connection>,
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
    let server_t: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let client_t: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });
    let client = Arc::new(Connection::new(client_t));
    let rx = client.start();
    (client, rx, fixture)
}

const T: Duration = Duration::from_secs(10);

/// Helper: initialize + start agent with optional prompt, return session_id.
async fn init_and_start(
    conn: &Connection,
    _rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    prompt: Option<&str>,
) -> String {
    let _ = tokio::time::timeout(
        T,
        conn.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();

    let mut params = serde_json::json!({});
    if let Some(p) = prompt {
        params["prompt"] = serde_json::Value::String(p.into());
    }
    let resp = tokio::time::timeout(T, conn.send_request(methods::AGENT_START.name, params))
        .await
        .unwrap()
        .unwrap();
    resp["session_id"].as_str().unwrap().to_string()
}

/// Helper: start agent (already initialized), return session_id.
async fn start_only(conn: &Connection, prompt: Option<&str>) -> String {
    let mut params = serde_json::json!({});
    if let Some(p) = prompt {
        params["prompt"] = serde_json::Value::String(p.into());
    }
    let resp = tokio::time::timeout(T, conn.send_request(methods::AGENT_START.name, params))
        .await
        .unwrap()
        .unwrap();
    resp["session_id"].as_str().unwrap().to_string()
}

/// Helper: drain events until Finished or AwaitingInput.
async fn drain_until_idle(rx: &mut tokio::sync::mpsc::Receiver<Incoming>) {
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<loopal_protocol::AgentEvent>(params) {
                        match ev.payload {
                            loopal_protocol::AgentEventPayload::Finished
                            | loopal_protocol::AgentEventPayload::AwaitingInput => return,
                            _ => {}
                        }
                    }
                }
            }
            _ => return,
        }
    }
}

/// dispatch_loop: Interactive session-1 → interrupted by agent/start → Session-2.
/// Note: non-interactive sessions now exit after completion (Hub architecture),
/// so session cycling only applies to interactive sessions receiving a new agent/start.
#[tokio::test]
async fn dispatch_loop_session_cycling() {
    use loopal_test_support::chunks;
    let (conn, mut rx, _f) = start_test_server_with_calls(vec![
        chunks::text_turn("reply-1"),
        chunks::text_turn("reply-2"),
    ])
    .await;

    // Session 1 (interactive: no prompt → waits for input)
    let sid1 = init_and_start(&conn, &mut rx, None).await;
    drain_until_idle(&mut rx).await;

    // Session 2: agent/start while session-1 is waiting interrupts and chains
    let sid2 = start_only(&conn, Some("world")).await;
    drain_until_idle(&mut rx).await;

    assert_ne!(sid1, sid2, "sessions should have different IDs");
}

/// ForwardResult::NewStart: sending agent/start while session is active
/// interrupts current session and starts a new one.
#[tokio::test]
async fn forward_new_start_interrupts_active_session() {
    use loopal_test_support::chunks;
    let (conn, mut rx, _f) = start_test_server_with_calls(vec![
        chunks::text_turn("first"),
        chunks::text_turn("second"),
    ])
    .await;

    // Start interactive session (no prompt → waits for input)
    let sid1 = init_and_start(&conn, &mut rx, None).await;
    drain_until_idle(&mut rx).await;

    // While session-1 is waiting, send agent/start to create session-2
    let sid2 = start_only(&conn, Some("go")).await;
    drain_until_idle(&mut rx).await;

    assert_ne!(sid1, sid2);
}
