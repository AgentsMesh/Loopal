//! End-to-end test for `loopal --hub-only` lifecycle:
//! spawn → handshake → discovery record → token handoff via socket.
//!
//! Isolates `HOME` to a tempdir so the spawned hub writes its discovery
//! record + socket under the test's own directory tree and never
//! pollutes the developer's `~/.loopal/run/`.

#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::time::timeout;

const SPAWN_DEADLINE: Duration = Duration::from_secs(20);

fn binary_path() -> String {
    std::env::var("LOOPAL_BINARY").expect("LOOPAL_BINARY env required")
}

fn write_mock_fixture() -> tempfile::NamedTempFile {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(br#"[[{"type":"text","text":"ok"},{"type":"usage"},{"type":"done"}]]"#)
        .unwrap();
    f.flush().unwrap();
    f
}

#[tokio::test]
async fn hub_only_handshake_writes_discovery_and_socket() {
    let fixture = write_mock_fixture();
    let home = tempfile::tempdir().expect("tempdir for HOME");
    let mut child = Command::new(binary_path())
        .arg("--hub-only")
        .env("HOME", home.path())
        .env("LOOPAL_TEST_PROVIDER", fixture.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn loopal --hub-only");
    let pid = child.id().expect("child pid");
    let stdout = child.stdout.take().expect("captured stdout");

    let line = timeout(SPAWN_DEADLINE, read_first_line(stdout))
        .await
        .expect("handshake timeout")
        .expect("read line");

    assert!(
        line.starts_with("LOOPAL_HUB "),
        "expected LOOPAL_HUB prefix, got {line:?}"
    );
    let mut parts = line["LOOPAL_HUB ".len()..].splitn(3, ' ');
    let addr = parts.next().expect("addr").to_string();
    let token = parts.next().expect("token").to_string();
    let session_id = parts.next().expect("session_id").trim().to_string();
    assert!(addr.starts_with("127.0.0.1:"), "unexpected addr {addr}");
    assert!(!token.is_empty(), "empty token");
    assert!(!session_id.is_empty(), "empty session_id");

    let run_dir = home.path().join(".loopal").join("run");
    let record_path = run_dir.join(format!("{pid}.json"));
    assert!(record_path.exists(), "{record_path:?} should exist");
    assert_record_owner_only(&record_path);
    assert_record_fields(&record_path, pid, &addr, &session_id);

    let socket_path = run_dir.join(format!("{pid}.sock"));
    assert!(socket_path.exists(), "{socket_path:?} should exist");

    let stream = UnixStream::connect(&socket_path)
        .await
        .expect("connect token socket");
    let mut reader = BufReader::new(stream);
    let mut got = String::new();
    reader
        .read_line(&mut got)
        .await
        .expect("read token from socket");
    assert_eq!(got.trim(), token, "socket token must match handshake token");

    drop(child);
}

async fn read_first_line(stdout: tokio::process::ChildStdout) -> std::io::Result<String> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    Ok(line.trim_end().to_string())
}

fn assert_record_owner_only(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let perms = std::fs::metadata(path).expect("stat record").permissions();
    let mode = perms.mode() & 0o777;
    assert_eq!(mode, 0o600, "record perms must be 0600, got {mode:o}");
}

fn assert_record_fields(
    path: &std::path::Path,
    expected_pid: u32,
    expected_addr: &str,
    expected_session_id: &str,
) {
    let body = std::fs::read_to_string(path).expect("read record");
    let json: serde_json::Value = serde_json::from_str(&body).expect("parse record");
    assert_eq!(
        json["pid"].as_u64(),
        Some(u64::from(expected_pid)),
        "pid mismatch in {body}"
    );
    assert_eq!(
        json["tcp_addr"].as_str(),
        Some(expected_addr),
        "tcp_addr mismatch in {body}"
    );
    assert_eq!(
        json["root_session_id"].as_str(),
        Some(expected_session_id),
        "root_session_id mismatch in {body}"
    );
    assert!(
        json["cwd"].as_str().is_some_and(|s| !s.is_empty()),
        "cwd must be non-empty in {body}"
    );
    assert!(
        json["started_at"].as_str().is_some_and(|s| !s.is_empty()),
        "started_at must be non-empty in {body}"
    );
}
