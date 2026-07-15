//! E2E smoke test for the bootstrap typestate chain.
//!
//! Drives a real `loopal --hub-only` subprocess and validates that
//! `Bootstrap<HubUninit → … → Ready>` reaches the Ready terminal under
//! the layered handshake budget (proxy 8s < start_agent 20s < handshake
//! 30s). A regression that re-introduces the phase 1 reverse-channel
//! deadlock — or skips `register_handlers` / `spawn_agent_process` — would
//! either fail to emit `LOOPAL_HUB_ALIVE` or never reach `LOOPAL_HUB_READY`.

#![cfg(unix)]

use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

const ALIVE_BUDGET: Duration = Duration::from_secs(3);
const READY_BUDGET: Duration = Duration::from_secs(8);

fn binary_path() -> String {
    std::env::var("LOOPAL_BINARY").expect("LOOPAL_BINARY env required")
}

#[tokio::test]
async fn typestate_chain_reaches_ready_within_layered_budget() {
    let home = tempfile::tempdir().expect("tempdir for HOME");

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
    let mut reader = BufReader::new(stdout);

    let alive_start = Instant::now();
    let alive = read_until_prefix(&mut reader, &["LOOPAL_HUB_ALIVE ", "LOOPAL_HUB "])
        .await
        .expect("ALIVE/Legacy line must appear");
    let alive_elapsed = alive_start.elapsed();
    assert!(
        alive_elapsed < ALIVE_BUDGET,
        "ALIVE phase exceeded budget {}s: took {alive_elapsed:?} — \
         register_handlers/bind_listener regressed",
        ALIVE_BUDGET.as_secs()
    );
    assert!(
        alive.starts_with("LOOPAL_HUB_ALIVE ") || alive.starts_with("LOOPAL_HUB "),
        "unexpected ALIVE line: {alive:?}"
    );

    let ready_start = Instant::now();
    let ready_or_legacy = read_until_prefix(&mut reader, &["LOOPAL_HUB_READY ", "LOOPAL_HUB "])
        .await
        .expect("READY/Legacy line must appear");
    let ready_elapsed = ready_start.elapsed();
    assert!(
        ready_elapsed < READY_BUDGET,
        "READY phase exceeded budget {}s: took {ready_elapsed:?} — \
         spawn_agent_process/start_root_agent regressed",
        READY_BUDGET.as_secs()
    );
    assert!(
        ready_or_legacy.starts_with("LOOPAL_HUB_READY ")
            || ready_or_legacy.starts_with("LOOPAL_HUB "),
        "unexpected READY line: {ready_or_legacy:?}"
    );

    drop(child);
}

async fn read_until_prefix(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    prefixes: &[&str],
) -> Result<String, String> {
    let total_budget = Duration::from_secs(10);
    let read = async {
        loop {
            let mut line = String::new();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| format!("read_line error: {e}"))?;
            if n == 0 {
                return Err("subprocess stdout closed before expected prefix".into());
            }
            let trimmed = line.trim_end().to_string();
            if prefixes.iter().any(|p| trimmed.starts_with(p)) {
                return Ok(trimmed);
            }
        }
    };
    timeout(total_budget, read)
        .await
        .map_err(|_| "no matching prefix within total budget".to_string())?
}
