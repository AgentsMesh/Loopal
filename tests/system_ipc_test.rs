//! System integration tests — spawns real `loopal --serve` subprocess.
//!
//! These tests build the actual binary, spawn it as a child process with
//! `--test-provider` flag pointing to a JSON fixture, and verify the full
//! multi-process IPC pipeline end-to-end.

use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;

/// Path to the built binary. Checks LOOPAL_BINARY env var (Bazel), then
/// CARGO_BIN_EXE_loopal (Cargo).
fn binary_path() -> String {
    if let Ok(path) = std::env::var("LOOPAL_BINARY") {
        return path;
    }
    std::env::var("CARGO_BIN_EXE_loopal")
        .expect("Set LOOPAL_BINARY or CARGO_BIN_EXE_loopal to the loopal binary path")
}

/// Write a mock provider JSON fixture and return the path.
fn write_mock_fixture(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

const TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test]
async fn system_spawn_and_initialize() {
    let fixture = write_mock_fixture(
        r#"[[{"type":"text","text":"Hello!"},{"type":"usage"},{"type":"done"}]]"#,
    );

    let mut child = tokio::process::Command::new(binary_path())
        .arg("--serve")
        .arg("--test-provider")
        .arg(fixture.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to spawn loopal --serve");

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let transport: std::sync::Arc<dyn loopal_ipc::transport::Transport> = std::sync::Arc::new(
        StdioTransport::new(Box::new(tokio::io::BufReader::new(stdout)), Box::new(stdin)),
    );
    let conn = std::sync::Arc::new(Connection::new(transport));
    let mut rx = conn.start();

    // Initialize
    let resp = tokio::time::timeout(
        TIMEOUT,
        conn.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resp["protocol_version"], 1);
    assert_eq!(resp["agent_info"]["name"], "loopal");

    // Start agent with prompt
    let resp = tokio::time::timeout(
        TIMEOUT,
        conn.send_request(
            methods::AGENT_START.name,
            serde_json::json!({"prompt": "say hello"}),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(resp.get("session_id").is_some());

    // Collect events until Finished or timeout
    let mut got_stream = false;
    let mut got_finished = false;
    let mut all_texts = String::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(event) = serde_json::from_value::<loopal_protocol::AgentEvent>(params)
                    {
                        match &event.payload {
                            loopal_protocol::AgentEventPayload::Stream { text } => {
                                all_texts.push_str(text);
                                got_stream = true;
                            }
                            loopal_protocol::AgentEventPayload::Finished => {
                                got_finished = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => break,
        }
    }

    assert!(
        got_stream,
        "should have received stream events, texts: '{all_texts}'"
    );
    assert!(got_finished, "should have received Finished event");

    // Process should exit cleanly
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    match status {
        Ok(Ok(s)) => assert!(s.success(), "child exited with: {s}"),
        Ok(Err(e)) => panic!("wait error: {e}"),
        Err(_) => {
            // Process may still be waiting for input — kill it
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}

#[tokio::test]
async fn system_process_isolation_survives_kill() {
    let fixture = write_mock_fixture(
        r#"[[{"type":"text","text":"thinking..."},{"type":"usage"},{"type":"done"}]]"#,
    );

    let mut child = tokio::process::Command::new(binary_path())
        .arg("--serve")
        .arg("--test-provider")
        .arg(fixture.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to spawn");

    let pid = child.id().expect("should have pid");
    assert!(pid > 0, "child should have a valid PID");

    // Kill the child — parent (this test process) should survive
    child.kill().await.unwrap();
    let status = child.wait().await.unwrap();
    assert!(!status.success(), "killed process should not be success");

    // We're still alive — test passes if we reach here
}
