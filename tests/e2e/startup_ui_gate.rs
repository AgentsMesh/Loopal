use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{HandshakeLine, TcpTransport};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;

#[tokio::test]
async fn registered_tui_transport_is_reused_across_alive_and_ready() {
    let home = tempfile::tempdir().expect("tempdir for HOME");
    let mut child = Command::new(super::binary_path())
        .args(["--hub-only", "--require-ui-ready"])
        .env("HOME", home.path())
        .env("LOOPAL_MCP_STARTUP_WAIT_SECS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn gated hub child");
    let stdout = child.stdout.take().expect("captured stdout");
    let mut lines = BufReader::new(stdout).lines();
    let alive = next_handshake(&mut lines, "alive").await;
    let HandshakeLine::Alive { addr, token } = alive else {
        panic!("expected ALIVE handshake")
    };

    let stream = TcpStream::connect(&addr).await.expect("connect gated hub");
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, mut incoming) = Connection::new(transport).into_listening();
    let registered = conn
        .send_request(
            methods::HUB_REGISTER.name,
            serde_json::json!({
                "name": "parent-tui",
                "token": token,
                "role": "ui_client",
                "capabilities": {
                    "permission": true,
                    "question": true,
                    "plan_approval": true,
                },
            }),
        )
        .await
        .expect("register parent TUI");
    assert_eq!(registered["ok"], true);

    assert!(matches!(
        next_handshake(&mut lines, "ready").await,
        HandshakeLine::Ready { .. }
    ));
    let event = timeout(Duration::from_secs(3), async {
        loop {
            match incoming.recv().await {
                Some(Incoming::Notification { method, params })
                    if method == methods::AGENT_EVENT.name =>
                {
                    return params;
                }
                Some(_) => {}
                None => panic!("registered TUI transport closed before initial event"),
            }
        }
    })
    .await
    .expect("initial event must use the pre-READY TUI transport");
    assert!(event.get("payload").is_some());

    conn.send_request(methods::HUB_SHUTDOWN.name, serde_json::json!({}))
        .await
        .expect("shut down gated hub");
    timeout(Duration::from_secs(8), child.wait())
        .await
        .expect("hub shutdown deadline")
        .expect("wait hub child");
}

async fn next_handshake(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    phase: &str,
) -> HandshakeLine {
    timeout(Duration::from_secs(10), async {
        loop {
            let line = lines
                .next_line()
                .await
                .expect("read handshake")
                .expect("stdout before handshake");
            if let Some(handshake) = HandshakeLine::parse(&line) {
                return handshake;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{phase} handshake deadline"))
}
