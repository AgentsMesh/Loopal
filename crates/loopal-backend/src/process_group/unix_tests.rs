use std::io;
use std::time::Duration;

use tokio::process::Command;

use super::{PlatformChild, failed, root_exited, signal_group};
use crate::process_group::{KillOutcome, Termination};

#[test]
fn failure_result_preserves_stage_and_has_no_exit_code() {
    let termination = failed("probe", io::Error::other("injected"));
    assert!(matches!(termination.outcome, KillOutcome::KillFailed(_)));
    assert!(format!("{:?}", termination.outcome).contains("probe"));
    assert_eq!(termination.exit_code, None);
}

#[test]
fn nonexistent_group_is_already_terminal() {
    signal_group(i32::MAX, libc::SIGTERM).unwrap();
    assert!(root_exited(i32::MAX).is_err());
}

#[tokio::test]
async fn terminate_reaps_an_already_exited_root() {
    let mut command = Command::new("sh");
    command.args(["-c", "exit 7"]);
    let mut child = PlatformChild::spawn(command).unwrap();
    child.wait_for_root_exit().await.unwrap();

    let result = child.terminate(Duration::from_secs(1)).await;
    assert_eq!(result.outcome, KillOutcome::Terminated);
    assert_eq!(result.exit_code, Some(7));
}

#[tokio::test]
async fn residual_cleanup_preserves_outcome_for_terminal_group() {
    let mut command = Command::new("sh");
    command.args(["-c", "exit 9"]);
    let mut child = PlatformChild::spawn(command).unwrap();
    child.wait_for_root_exit().await.unwrap();

    let result = child
        .kill_residual_and_reap(KillOutcome::Terminated, Duration::from_secs(1))
        .await;
    assert_eq!(result.outcome, KillOutcome::Terminated);
    assert_eq!(result.exit_code, Some(9));
}

#[tokio::test]
async fn invalid_signal_and_reap_timeout_fail_closed() {
    let mut command = Command::new("sh");
    command.args(["-c", "trap '' TERM; while :; do sleep 1; done"]);
    let mut child = PlatformChild::spawn(command).unwrap();

    assert!(signal_group(child.pgid, i32::MAX).is_err());
    let result: Termination = child.reap(KillOutcome::Killed, Duration::ZERO).await;
    assert!(matches!(result.outcome, KillOutcome::KillFailed(_)));

    let cleanup = child.terminate(Duration::from_millis(100)).await;
    assert!(!matches!(cleanup.outcome, KillOutcome::KillFailed(_)));
}
