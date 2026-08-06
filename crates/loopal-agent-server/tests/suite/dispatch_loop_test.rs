//! Integration tests for dispatch_loop session cycling and session_forward.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::{HangingProvider, MultiCallProvider};

type TestServer = (
    Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
    TestFixture,
    tokio::task::JoinHandle<()>,
);

async fn start_test_server(provider: Arc<dyn loopal_provider_api::Provider>) -> TestServer {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");

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
    let server = tokio::spawn(async move {
        loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir)
            .await
            .expect("test agent server failed");
    });
    let (client, rx) = Connection::new(client_t).into_listening();
    (client, rx, fixture, server)
}

async fn start_test_server_with_calls(
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> TestServer {
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    start_test_server(provider).await
}

const T: Duration = Duration::from_secs(10);

/// Helper: initialize + start agent with optional prompt, return session_id.
async fn init_and_start(
    conn: &Connection<Listening>,
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

    let mut params = serde_json::json!({"model": "claude-opus-4-8"});
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
async fn start_only(conn: &Connection<Listening>, prompt: Option<&str>) -> String {
    let mut params = serde_json::json!({"model": "claude-opus-4-8"});
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
                if method == methods::AGENT_EVENT.name
                    && let Ok(ev) = serde_json::from_value::<loopal_protocol::AgentEvent>(params)
                {
                    match ev.payload {
                        loopal_protocol::AgentEventPayload::Finished
                        | loopal_protocol::AgentEventPayload::AwaitingInput => return,
                        _ => {}
                    }
                }
            }
            _ => return,
        }
    }
}

async fn wait_for_event(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    predicate: impl Fn(&AgentEventPayload) -> bool,
) {
    tokio::time::timeout(T, async {
        while let Some(Incoming::Notification { method, params }) = rx.recv().await {
            if method != methods::AGENT_EVENT.name {
                continue;
            }
            let event: AgentEvent = serde_json::from_value(params).unwrap();
            if predicate(&event.payload) {
                return;
            }
        }
        panic!("agent event stream closed");
    })
    .await
    .expect("expected agent event");
}

async fn send_message(conn: &Connection<Listening>, text: &str) {
    let envelope = Envelope::new(MessageSource::Human, "main", text);
    tokio::time::timeout(
        T,
        conn.send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::to_value(envelope).unwrap(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
}

/// dispatch_loop: Interactive session-1 → interrupted by agent/start → Session-2.
/// Note: non-interactive sessions now exit after completion (Hub architecture),
/// so session cycling only applies to interactive sessions receiving a new agent/start.
#[tokio::test]
async fn dispatch_loop_session_cycling() {
    use loopal_test_support::chunks;
    let (conn, mut rx, _f, _server) = start_test_server_with_calls(vec![
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
    let (conn, mut rx, _f, _server) = start_test_server_with_calls(vec![
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

/// Closing the client transport (EOF) causes the server to exit cleanly
/// within a bounded time, without relying on process-level SIGKILL.
#[tokio::test]
async fn forward_loop_eof_exits_cleanly() {
    use loopal_test_support::chunks;

    // Server with one interactive session (no prompt → waits for input)
    let (conn, mut rx, _f, _server) =
        start_test_server_with_calls(vec![chunks::text_turn("reply")]).await;

    let _sid = init_and_start(&conn, &mut rx, None).await;
    drain_until_idle(&mut rx).await;

    // Close the client transport → server should see EOF and exit
    conn.close().await;

    // Server must exit within 2 seconds (internal budget is ~1s).
    // If the shutdown path is broken, this timeout fires.
    let deadline = tokio::time::timeout(Duration::from_secs(2), async {
        // After close, the reader loop sees EOF and rx closes.
        while rx.recv().await.is_some() {}
    })
    .await;

    assert!(
        deadline.is_ok(),
        "server should exit promptly after client EOF"
    );
}

/// `agent/shutdown` is process-scoped even when a persistent runtime is in a
/// turn. Its ACK must not leave `forward_loop` waiting for AwaitingInput.
#[tokio::test]
async fn shutdown_during_active_persistent_session_exits_server() {
    let provider = Arc::new(HangingProvider) as Arc<dyn loopal_provider_api::Provider>;
    let (conn, mut rx, _fixture, server) = start_test_server(provider).await;

    init_and_start(&conn, &mut rx, None).await;
    drain_until_idle(&mut rx).await;
    send_message(&conn, "stay active").await;
    wait_for_event(&mut rx, |event| matches!(event, AgentEventPayload::Running)).await;

    let ack = tokio::time::timeout(
        Duration::from_secs(2),
        conn.send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({})),
    )
    .await
    .expect("active-session shutdown ACK timed out")
    .expect("active-session shutdown failed");
    assert_eq!(ack["ok"], true);

    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server loop stayed alive after acknowledged shutdown")
        .expect("server task panicked");
}

/// `agent/interrupt` is turn-scoped: after its ACK the persistent session can
/// accept another message, and only an explicit shutdown exits the server.
#[tokio::test]
async fn interrupt_cancels_turn_without_exiting_server() {
    let provider = Arc::new(HangingProvider) as Arc<dyn loopal_provider_api::Provider>;
    let (conn, mut rx, _fixture, mut server) = start_test_server(provider).await;

    init_and_start(&conn, &mut rx, None).await;
    drain_until_idle(&mut rx).await;
    send_message(&conn, "first turn").await;
    wait_for_event(&mut rx, |event| matches!(event, AgentEventPayload::Running)).await;

    let ack = tokio::time::timeout(
        Duration::from_secs(2),
        conn.send_request(methods::AGENT_INTERRUPT.name, serde_json::json!({})),
    )
    .await
    .expect("interrupt ACK timed out")
    .expect("interrupt failed");
    assert_eq!(ack["ok"], true);
    wait_for_event(&mut rx, |event| {
        matches!(event, AgentEventPayload::Interrupted)
    })
    .await;
    assert!(!server.is_finished(), "interrupt exited the server loop");

    send_message(&conn, "second turn").await;
    wait_for_event(&mut rx, |event| matches!(event, AgentEventPayload::Running)).await;
    assert!(
        !server.is_finished(),
        "persistent session did not survive interrupt"
    );

    conn.send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({}))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), &mut server)
        .await
        .expect("cleanup shutdown did not exit server")
        .expect("server task panicked");
}
