//! End-to-end contract tests for the stable `loopal desktop serve` entry.

#![cfg(unix)]

#[path = "session_phase.rs"]
mod session_phase;
#[path = "support.rs"]
mod support;

use std::process::Stdio;
use std::time::Duration;

use loopal_ipc::{
    DESKTOP_CAPABILITY_HUB_UI, DESKTOP_CAPABILITY_WORKSPACE, DESKTOP_TRANSPORT, DesktopHandshake,
    DesktopHandshakeEvent,
};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use support::{
    EXIT_DEADLINE, STARTUP_DEADLINE, assert_common_metadata, read_alive_and_ready,
    request_hub_shutdown, spawn_desktop, wait_for_registration_withdrawal, write_mock_fixture,
};

#[tokio::test]
async fn desktop_serve_emits_versioned_handshake_and_exits_with_parent() {
    let home = tempfile::tempdir().expect("temp HOME");
    let fixture = write_mock_fixture();

    let mut parent = Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn parent sentinel");
    let parent_pid = parent.id().expect("parent pid");

    let mut desktop = spawn_desktop(&home, &fixture, parent_pid);
    let desktop_pid = desktop.id().expect("desktop pid");
    let stdout = desktop.stdout.take().expect("captured Desktop stdout");
    let (alive, ready, observed_lines) = timeout(STARTUP_DEADLINE, read_alive_and_ready(stdout))
        .await
        .expect("Desktop handshake deadline")
        .expect("read Desktop handshake");

    assert!(
        observed_lines
            .iter()
            .all(|line| !line.starts_with("LOOPAL_HUB")),
        "stable Desktop command must not leak legacy positional handshakes: {observed_lines:?}"
    );
    assert_common_metadata(&alive, desktop_pid, parent_pid);
    assert_common_metadata(&ready, desktop_pid, parent_pid);

    let (addr, token, transport, capabilities) = match alive.event {
        DesktopHandshakeEvent::Alive {
            addr,
            token,
            transport,
            capabilities,
        } => (addr, token, transport, capabilities),
        other => panic!("expected alive event, got {other:?}"),
    };
    assert!(addr.starts_with("127.0.0.1:"), "unexpected Hub addr {addr}");
    assert!(!token.is_empty(), "Hub token must not be empty");
    assert_eq!(transport, DESKTOP_TRANSPORT);
    assert_eq!(
        capabilities,
        vec![
            DESKTOP_CAPABILITY_HUB_UI.to_string(),
            DESKTOP_CAPABILITY_WORKSPACE.to_string(),
        ]
    );
    session_phase::assert_fresh_success_order(&observed_lines, desktop_pid, parent_pid);
    match ready.event {
        DesktopHandshakeEvent::Ready { session_id } => {
            assert!(!session_id.is_empty(), "root session id must not be empty")
        }
        other => panic!("expected ready event, got {other:?}"),
    }

    parent.kill().await.expect("terminate parent sentinel");
    let _ = parent.wait().await;
    wait_for_registration_withdrawal(&home, desktop_pid).await;

    let status = timeout(EXIT_DEADLINE, desktop.wait())
        .await
        .expect("Desktop Host must observe parent exit")
        .expect("wait Desktop Host");
    assert!(status.success(), "Desktop Host exit was {status}");

    let run_dir = home.path().join(".loopal").join("run");
    assert!(!run_dir.join(format!("{desktop_pid}.json")).exists());
    assert!(!run_dir.join(format!("{desktop_pid}.sock")).exists());
}

#[tokio::test]
async fn hub_shutdown_withdraws_registration_before_exit() {
    let home = tempfile::tempdir().expect("temp HOME");
    let fixture = write_mock_fixture();
    let mut parent = Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn parent sentinel");
    let parent_pid = parent.id().expect("parent pid");
    let mut desktop = spawn_desktop(&home, &fixture, parent_pid);
    let desktop_pid = desktop.id().expect("desktop pid");
    let stdout = desktop.stdout.take().expect("captured Desktop stdout");
    let (alive, _, _) = timeout(STARTUP_DEADLINE, read_alive_and_ready(stdout))
        .await
        .expect("Desktop handshake deadline")
        .expect("read Desktop handshake");
    let DesktopHandshakeEvent::Alive { addr, token, .. } = alive.event else {
        panic!("expected alive handshake")
    };

    request_hub_shutdown(&addr, &token).await;
    wait_for_registration_withdrawal(&home, desktop_pid).await;
    let status = timeout(EXIT_DEADLINE, desktop.wait())
        .await
        .expect("Desktop Host shutdown deadline")
        .expect("wait Desktop Host");
    assert!(status.success(), "Desktop Host exit was {status}");
    parent.kill().await.expect("terminate parent sentinel");
    let _ = parent.wait().await;
}

#[tokio::test]
async fn missing_parent_is_reported_as_structured_handshake_error() {
    let home = tempfile::tempdir().expect("temp HOME");
    let fixture = write_mock_fixture();
    let missing_pid = 2_000_000_000_u32;
    let mut desktop = spawn_desktop(&home, &fixture, missing_pid);
    let desktop_pid = desktop.id().expect("desktop pid");
    let stdout = desktop.stdout.take().expect("captured Desktop stdout");
    let mut reader = BufReader::new(stdout);
    let handshake = timeout(Duration::from_secs(3), async {
        loop {
            let mut line = String::new();
            let read = reader.read_line(&mut line).await?;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "stdout closed before structured error",
                ));
            }
            if let Some(handshake) =
                DesktopHandshake::parse(&line).map_err(std::io::Error::other)?
            {
                break Ok(handshake);
            }
        }
    })
    .await
    .expect("structured error deadline")
    .expect("read structured error");

    assert_common_metadata(&handshake, desktop_pid, missing_pid);
    match handshake.event {
        DesktopHandshakeEvent::Error { code, message } => {
            assert_eq!(code, "invalid_parent_process");
            assert!(message.contains("is not running"), "unexpected {message:?}");
        }
        other => panic!("expected error event, got {other:?}"),
    }

    let status = timeout(Duration::from_secs(3), desktop.wait())
        .await
        .expect("invalid-parent process must exit promptly")
        .expect("wait Desktop Host");
    assert!(
        !status.success(),
        "invalid parent must be a startup failure"
    );
}
