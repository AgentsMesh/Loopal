use std::process::Stdio;

use loopal_ipc::{
    DESKTOP_EVENT_PREFIX, DESKTOP_HANDSHAKE_PREFIX, DesktopHandshake, DesktopHandshakeEvent,
};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio::time::timeout;

use super::support::{
    EXIT_DEADLINE, STARTUP_DEADLINE, assert_common_metadata, read_alive_and_ready,
    request_hub_shutdown, spawn_desktop_with_resume, spawn_desktop_with_root_timeout,
    write_mock_fixture,
};

pub(super) fn assert_fresh_success_order(lines: &[String], pid: u32, parent_pid: u32) {
    let records = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            DesktopHandshake::parse(line)
                .expect("valid Desktop protocol record")
                .map(|record| (index, record))
        })
        .collect::<Vec<_>>();
    let (alive_index, _) = records
        .iter()
        .find(|(_, record)| matches!(record.event, DesktopHandshakeEvent::Alive { .. }))
        .expect("ALIVE record");
    let (created_index, created) = records
        .iter()
        .find(|(_, record)| matches!(record.event, DesktopHandshakeEvent::SessionCreated { .. }))
        .expect("SESSION_CREATED record");
    let (ready_index, ready) = records
        .iter()
        .find(|(_, record)| matches!(record.event, DesktopHandshakeEvent::Ready { .. }))
        .expect("READY record");
    assert!(alive_index < created_index && created_index < ready_index);
    assert_common_metadata(created, pid, parent_pid);
    let DesktopHandshakeEvent::SessionCreated { session_id } = &created.event else {
        unreachable!()
    };
    let DesktopHandshakeEvent::Ready {
        session_id: ready_id,
    } = &ready.event
    else {
        unreachable!()
    };
    assert_eq!(session_id, ready_id);
}

#[tokio::test]
async fn resumed_desktop_does_not_emit_session_created() {
    let home = tempfile::tempdir().expect("temp HOME");
    let fixture = write_mock_fixture();
    let mut parent = spawn_parent();
    let parent_pid = parent.id().expect("parent pid");

    let mut fresh = spawn_desktop_with_resume(&home, &fixture, parent_pid, None);
    let stdout = fresh.stdout.take().expect("fresh stdout");
    let (alive, ready, _) = timeout(STARTUP_DEADLINE, read_alive_and_ready(stdout))
        .await
        .expect("fresh deadline")
        .expect("fresh handshakes");
    let session_id = ready_session_id(ready);
    let (addr, token) = connection(alive);
    request_hub_shutdown(&addr, &token).await;
    wait_success(&mut fresh).await;

    let mut resumed = spawn_desktop_with_resume(&home, &fixture, parent_pid, Some(&session_id));
    let stdout = resumed.stdout.take().expect("resume stdout");
    let (alive, ready, lines) = timeout(STARTUP_DEADLINE, read_alive_and_ready(stdout))
        .await
        .expect("resume deadline")
        .expect("resume handshakes");
    assert_eq!(ready_session_id(ready), session_id);
    assert!(
        !lines
            .iter()
            .any(|line| line.starts_with(DESKTOP_EVENT_PREFIX))
    );
    let (addr, token) = connection(alive);
    request_hub_shutdown(&addr, &token).await;
    wait_success(&mut resumed).await;
    parent.kill().await.expect("terminate parent");
}

#[tokio::test]
async fn root_view_failure_follows_fresh_session_marker() {
    let home = tempfile::tempdir().expect("temp HOME");
    let fixture = write_mock_fixture();
    let mut parent = spawn_parent();
    let parent_pid = parent.id().expect("parent pid");
    let mut desktop = spawn_desktop_with_root_timeout(&home, &fixture, parent_pid);
    let stdout = desktop.stdout.take().expect("captured stdout");
    let lines = timeout(STARTUP_DEADLINE, read_until_root_error(stdout))
        .await
        .expect("root-view error deadline");
    let alive = phase_index(&lines, "alive", DESKTOP_HANDSHAKE_PREFIX);
    let created = phase_index(&lines, "session_created", DESKTOP_EVENT_PREFIX);
    let error = phase_index(&lines, "error", DESKTOP_HANDSHAKE_PREFIX);
    assert!(alive < created && created < error);
    assert!(!lines.iter().any(|line| line.contains(r#""phase":"ready""#)));
    let status = timeout(EXIT_DEADLINE, desktop.wait())
        .await
        .expect("failed Desktop exit deadline")
        .expect("wait failed Desktop");
    assert!(!status.success());
    parent.kill().await.expect("terminate parent");
}

async fn read_until_root_error(stdout: tokio::process::ChildStdout) -> Vec<String> {
    let mut reader = BufReader::new(stdout);
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).await.expect("read stdout"), 0);
        lines.push(line.trim_end().to_string());
        let Some(record) = DesktopHandshake::parse(&line).expect("valid protocol") else {
            continue;
        };
        if let DesktopHandshakeEvent::Error { code, .. } = record.event {
            assert_eq!(code, "root_view_not_ready");
            return lines;
        }
    }
}

fn phase_index(lines: &[String], phase: &str, prefix: &str) -> usize {
    let needle = format!(r#""phase":"{phase}""#);
    lines
        .iter()
        .position(|line| line.starts_with(prefix) && line.contains(&needle))
        .unwrap_or_else(|| panic!("missing {phase} in {lines:?}"))
}

fn connection(handshake: DesktopHandshake) -> (String, String) {
    match handshake.event {
        DesktopHandshakeEvent::Alive { addr, token, .. } => (addr, token),
        other => panic!("expected ALIVE, got {other:?}"),
    }
}

fn ready_session_id(handshake: DesktopHandshake) -> String {
    match handshake.event {
        DesktopHandshakeEvent::Ready { session_id } => session_id,
        other => panic!("expected READY, got {other:?}"),
    }
}

fn spawn_parent() -> Child {
    Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn parent sentinel")
}

async fn wait_success(child: &mut Child) {
    let status = timeout(EXIT_DEADLINE, child.wait())
        .await
        .expect("Desktop exit deadline")
        .expect("wait Desktop");
    assert!(status.success(), "Desktop exit was {status}");
}
