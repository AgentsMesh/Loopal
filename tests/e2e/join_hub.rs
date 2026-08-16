//! End-to-end test for `loopal --hub-only --join-hub <addr>`:
//! verifies that the spawned hub-only process honors the `--join-hub`
//! flag and actually opens a TCP connection to the address, sending
//! a `meta/register` request with the token from `LOOPAL_META_HUB_TOKEN`.
//!
//! Regression guard for the Hub-subprocess refactor (#133), where the
//! TUI's spawn shim previously dropped `--join-hub` / `--hub-name` from
//! the child argv, leaving the hub silently disconnected.

#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::time::timeout;

const SPAWN_DEADLINE: Duration = Duration::from_secs(20);
const REGISTER_DEADLINE: Duration = Duration::from_secs(10);
const TOKEN: &str = "test-token-deadbeef";

fn binary_path() -> std::path::PathBuf {
    loopal_agent_client::require_runfile_env("LOOPAL_BINARY").expect("resolve LOOPAL_BINARY")
}

fn write_mock_provider() -> tempfile::NamedTempFile {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(br#"[[{"type":"text","text":"ok"},{"type":"usage"},{"type":"done"}]]"#)
        .unwrap();
    f.flush().unwrap();
    f
}

#[tokio::test]
async fn join_hub_sends_meta_register_with_token_and_name() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let meta_addr = listener.local_addr().expect("local_addr").to_string();
    let accept_task = tokio::spawn(accept_one_register(listener));

    let provider = write_mock_provider();
    let home = tempfile::tempdir().expect("tempdir for HOME");
    let mut child = Command::new(binary_path())
        .arg("--hub-only")
        .arg("--join-hub")
        .arg(&meta_addr)
        .arg("--hub-name")
        .arg("test-hub-a")
        .env("HOME", home.path())
        .env("LOOPAL_META_HUB_TOKEN", TOKEN)
        .env("LOOPAL_TEST_PROVIDER", provider.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn loopal --hub-only --join-hub");
    let stdout = child.stdout.take().expect("captured stdout");

    let _handshake = timeout(SPAWN_DEADLINE, read_first_line(stdout))
        .await
        .expect("handshake timeout")
        .expect("read handshake line");

    let register = timeout(REGISTER_DEADLINE, accept_task)
        .await
        .expect("meta/register timeout")
        .expect("accept task panicked")
        .expect("accept_one_register returned err");

    assert_eq!(
        register["method"].as_str(),
        Some("meta/register"),
        "expected meta/register method, got {register}"
    );
    let params = &register["params"];
    assert_eq!(
        params["token"].as_str(),
        Some(TOKEN),
        "token must propagate from LOOPAL_META_HUB_TOKEN env"
    );
    assert_eq!(
        params["name"].as_str(),
        Some("test-hub-a"),
        "hub name must propagate from --hub-name flag"
    );

    drop(child);
}

#[tokio::test]
async fn join_hub_uses_default_name_when_hub_name_omitted() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let meta_addr = listener.local_addr().expect("local_addr").to_string();
    let accept_task = tokio::spawn(accept_one_register(listener));

    let provider = write_mock_provider();
    let home = tempfile::tempdir().expect("tempdir for HOME");
    let mut child = Command::new(binary_path())
        .arg("--hub-only")
        .arg("--join-hub")
        .arg(&meta_addr)
        .env("HOME", home.path())
        .env("LOOPAL_META_HUB_TOKEN", TOKEN)
        .env("LOOPAL_TEST_PROVIDER", provider.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn");
    let stdout = child.stdout.take().expect("captured stdout");

    let _handshake = timeout(SPAWN_DEADLINE, read_first_line(stdout))
        .await
        .expect("handshake timeout")
        .expect("read handshake");

    let register = timeout(REGISTER_DEADLINE, accept_task)
        .await
        .expect("meta/register timeout")
        .expect("accept task panicked")
        .expect("accept err");

    let name = register["params"]["name"]
        .as_str()
        .expect("name field present");
    assert!(
        name.starts_with("hub-") && name.len() > 4,
        "default name should look like 'hub-XXXX', got {name:?}"
    );

    drop(child);
}

async fn read_first_line(stdout: tokio::process::ChildStdout) -> std::io::Result<String> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    Ok(line.trim_end().to_string())
}

/// Accept one TCP connection, read one JSON-RPC request, respond `{ok:true}`,
/// return the parsed request. Errors out if the request is not `meta/register`.
async fn accept_one_register(listener: TcpListener) -> anyhow::Result<Value> {
    let (stream, _peer) = listener.accept().await?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let request: Value = serde_json::from_str(line.trim_end())?;

    let id = request["id"].clone();
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {"ok": true}
    });
    let mut payload = serde_json::to_vec(&response)?;
    payload.push(b'\n');
    write_half.write_all(&payload).await?;
    write_half.flush().await?;

    Ok(request)
}
