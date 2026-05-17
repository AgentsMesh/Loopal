#![cfg(unix)]

use std::time::Duration;

use loopal_backend::process_group::{capture_pgid, configure_process_group};
use loopal_backend::{KillOutcome, SpawnedChild, kill_process_group};
use tokio::process::Command;

#[tokio::test]
async fn killpg_terminates_grandchildren_in_same_group() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("(sleep 30 & sleep 30 & wait) </dev/null >/dev/null 2>&1")
        .kill_on_drop(true);
    configure_process_group(&mut cmd);

    let mut child = cmd.spawn().expect("spawn shell");
    let pgid = capture_pgid(&child).expect("pgid available on unix");
    let parent_pid = pgid;

    tokio::time::sleep(Duration::from_millis(150)).await;
    let descendants_before = count_pids_in_pgroup(parent_pid);
    assert!(
        descendants_before >= 3,
        "expected at least sh + 2 sleeps in pgroup; got {descendants_before}"
    );

    let outcome = kill_process_group(Some(pgid), &mut child, Duration::from_millis(500)).await;
    assert!(
        matches!(
            outcome,
            KillOutcome::Terminated | KillOutcome::Killed | KillOutcome::FallbackChild
        ),
        "expected successful kill outcome, got {outcome:?}"
    );

    tokio::time::sleep(Duration::from_millis(200)).await;
    let descendants_after = count_pids_in_pgroup(parent_pid);
    assert_eq!(
        descendants_after, 0,
        "all processes in the group must be reaped"
    );
}

#[tokio::test]
async fn spawned_child_struct_carries_pgid_on_unix() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("sleep 5").kill_on_drop(true);
    configure_process_group(&mut cmd);

    let child = cmd.spawn().expect("spawn shell");
    let pgid = capture_pgid(&child);
    assert!(pgid.is_some(), "unix path must populate pgid");

    let mut spawned = SpawnedChild { child, pgid };
    let _ = spawned.child.start_kill();
    let _ = spawned.child.wait().await;
}

fn count_pids_in_pgroup(pgid: i32) -> usize {
    let out = match std::process::Command::new("ps")
        .args(["-o", "pgid="])
        .arg("-g")
        .arg(pgid.to_string())
        .output()
    {
        Ok(o) => o,
        Err(_) => return 0,
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}
