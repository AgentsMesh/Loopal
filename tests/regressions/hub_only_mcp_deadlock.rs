//! Deadlock regression: hub-only must finish handshake within 5s even when a
//! configured MCP server is unresponsive. Guards the hub_bootstrap ordering
//! that starts the reverse-IPC consumer before sending `agent/start`.

#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

const HANDSHAKE_BUDGET: Duration = Duration::from_secs(5);

fn binary_path() -> std::path::PathBuf {
    loopal_agent_client::require_runfile_env("LOOPAL_BINARY").expect("resolve LOOPAL_BINARY")
}

fn write_settings_with_unresponsive_mcp(home_dir: &std::path::Path) -> std::io::Result<()> {
    let loopal_dir = home_dir.join(".loopal");
    std::fs::create_dir_all(&loopal_dir)?;
    let settings = serde_json::json!({
        "providers": {
            "anthropic": {
                "api_key": "test-key-not-used",
                "base_url": "http://127.0.0.1:1/fake"
            }
        },
        "mcp_servers": {
            "unresponsive-mock": {
                "type": "stdio",
                "command": "sleep",
                "args": ["60"],
                "timeout_ms": 60_000
            }
        }
    });
    std::fs::write(
        loopal_dir.join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
}

#[tokio::test]
async fn hub_only_handshake_within_5s_even_with_unresponsive_mcp() {
    let home = tempfile::tempdir().expect("tempdir for HOME");
    write_settings_with_unresponsive_mcp(home.path()).expect("write settings");

    let mut child = Command::new(binary_path())
        .arg("--hub-only")
        .env("HOME", home.path())
        .env("LOOPAL_MCP_STARTUP_WAIT_SECS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn loopal --hub-only");
    let stdout = child.stdout.take().expect("captured stdout");

    let line_result = timeout(HANDSHAKE_BUDGET, read_first_line(stdout)).await;

    match line_result {
        Ok(Ok(line)) => {
            let recognised = line.starts_with("LOOPAL_HUB ")
                || line.starts_with("LOOPAL_HUB_ALIVE ")
                || line.starts_with("LOOPAL_HUB_READY ");
            assert!(
                recognised,
                "expected LOOPAL_HUB[_ALIVE|_READY] prefix within {}s, got: {line:?}",
                HANDSHAKE_BUDGET.as_secs()
            );
        }
        Ok(Err(e)) => panic!("read_first_line failed: {e}"),
        Err(_) => panic!(
            "hub-only failed to emit handshake line within {}s — \
             reverse-IPC deadlock detected (root cause: hub_bootstrap starts \
             start_agent_io after client.start_agent.await)",
            HANDSHAKE_BUDGET.as_secs()
        ),
    }

    drop(child);
}

async fn read_first_line(stdout: tokio::process::ChildStdout) -> std::io::Result<String> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    Ok(line.trim_end().to_string())
}
