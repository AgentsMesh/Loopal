#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

use loopal_backend::{KillOutcome, SpawnedChild};
use tokio::process::Command;

use crate::process_test_support::{
    remove_pid_file, unique_pid_path, wait_for_file, wait_for_pid, wait_until_terminal,
};

fn descendant_command(pid_file: &std::path::Path, leader_exits: bool) -> Command {
    let mut command = Command::new("sh");
    let end = if leader_exits { "exit 0" } else { "wait" };
    command
        .arg("-c")
        .arg(format!(
            "(trap '' TERM; while :; do sleep 1; done) & child=$!; echo $child > \"$PID_FILE\"; {end}"
        ))
        .env("PID_FILE", pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[tokio::test]
async fn terminate_escalates_and_proves_descendant_terminal() {
    let pid_file = unique_pid_path("terminate-tree");
    let mut spawned = SpawnedChild::spawn(descendant_command(&pid_file, false)).unwrap();
    let descendant = wait_for_pid(&pid_file).await;

    let termination = spawned.terminate(Duration::from_millis(100)).await;
    assert_eq!(termination.outcome, KillOutcome::Killed);
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}

#[tokio::test]
async fn natural_leader_exit_kills_live_descendant_before_returning() {
    let pid_file = unique_pid_path("natural-tree");
    let mut spawned = SpawnedChild::spawn(descendant_command(&pid_file, true)).unwrap();
    let descendant = wait_for_pid(&pid_file).await;

    let status = tokio::time::timeout(Duration::from_secs(3), spawned.wait())
        .await
        .expect("tree wait bounded")
        .expect("tree wait succeeds");
    assert!(status.success());
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}

#[tokio::test]
async fn terminate_grace_covers_descendant_after_leader_exits() {
    let pid_file = unique_pid_path("grace-tree");
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(
            "(trap 'sleep 0.2; exit 0' TERM; while :; do sleep 1; done) & child=$!; echo $child > \"$PID_FILE\"; wait",
        )
        .env("PID_FILE", &pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut spawned = SpawnedChild::spawn(command).unwrap();
    let descendant = wait_for_pid(&pid_file).await;
    let started = std::time::Instant::now();

    let termination = spawned.terminate(Duration::from_secs(2)).await;
    assert_eq!(termination.outcome, KillOutcome::Terminated);
    assert!(started.elapsed() >= Duration::from_millis(180));
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}

#[tokio::test]
async fn cancelling_terminate_future_keeps_owner_armed() {
    let pid_file = unique_pid_path("cancel-terminate-tree");
    let term_file = unique_pid_path("cancel-terminate-ready");
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(
            "(trap 'printf ready > \"$TERM_FILE\"; while :; do :; done' TERM; while :; do :; done) & child=$!; echo $child > \"$PID_FILE\"; wait",
        )
        .env("PID_FILE", &pid_file)
        .env("TERM_FILE", &term_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let spawned = SpawnedChild::spawn(command).unwrap();
    let descendant = wait_for_pid(&pid_file).await;
    let task = tokio::spawn(async move {
        let mut spawned = spawned;
        spawned.terminate(Duration::from_secs(30)).await
    });
    wait_for_file(&term_file).await;

    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
    remove_pid_file(&term_file).await;
}

#[tokio::test]
async fn terminate_after_successful_wait_is_a_noop() {
    let mut command = Command::new("sh");
    command.args(["-c", "exit 7"]);
    let mut spawned = SpawnedChild::spawn(command).unwrap();

    let status = spawned.wait().await.unwrap();
    assert_eq!(status.code(), Some(7));
    let termination = spawned.terminate(Duration::ZERO).await;
    assert_eq!(termination.outcome, KillOutcome::Terminated);
    assert_eq!(termination.exit_code, None);
}

#[tokio::test]
async fn dropping_armed_owner_kills_descendant() {
    let pid_file = unique_pid_path("drop-tree");
    let spawned = SpawnedChild::spawn(descendant_command(&pid_file, false)).unwrap();
    let descendant = wait_for_pid(&pid_file).await;

    drop(spawned);
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}
