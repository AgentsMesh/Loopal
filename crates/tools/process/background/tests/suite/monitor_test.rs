#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use loopal_backend::process_group::{capture_pgid, configure_process_group};
use loopal_tool_background::monitor::{MonitorTiming, run_process_monitor};
use loopal_tool_background::{ControlSignal, StopOutcome, TaskStatus};
use tokio::sync::{mpsc, oneshot, watch};

fn test_timing() -> MonitorTiming {
    MonitorTiming {
        drainers_grace: Duration::from_millis(50),
        sigterm_grace: Duration::from_millis(500),
    }
}

async fn spawn_sleeper(secs: u32) -> (tokio::process::Child, Option<i32>) {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c")
        .arg(format!("sleep {secs}"))
        .kill_on_drop(true);
    configure_process_group(&mut cmd);
    let child = cmd.spawn().expect("spawn sleeper");
    let pgid = capture_pgid(&child);
    (child, pgid)
}

#[tokio::test]
async fn monitor_force_kills_when_control_tx_dropped() {
    let (child, pgid) = spawn_sleeper(30).await;
    let exit_code = Arc::new(AtomicI32::new(i32::MIN));
    let (status_tx, mut status_rx) = watch::channel(TaskStatus::Running);
    let (control_tx, control_rx) = mpsc::channel(4);

    let monitor = tokio::spawn(run_process_monitor(
        child,
        pgid,
        vec![],
        exit_code,
        status_tx,
        control_rx,
        test_timing(),
    ));

    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(control_tx);

    let wait = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if status_rx.changed().await.is_err() {
                return;
            }
            if status_rx.borrow().is_terminal() {
                return;
            }
        }
    });
    wait.await.expect("status should become terminal");
    assert_eq!(*status_rx.borrow(), TaskStatus::Killed);
    monitor.await.expect("monitor task panic");
}

#[tokio::test]
async fn monitor_returns_killed_outcome_to_stop_ack() {
    let (child, pgid) = spawn_sleeper(30).await;
    let exit_code = Arc::new(AtomicI32::new(i32::MIN));
    let (status_tx, status_rx) = watch::channel(TaskStatus::Running);
    let (control_tx, control_rx) = mpsc::channel(4);

    let monitor = tokio::spawn(run_process_monitor(
        child,
        pgid,
        vec![],
        exit_code.clone(),
        status_tx,
        control_rx,
        test_timing(),
    ));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let (ack_tx, ack_rx) = oneshot::channel();
    control_tx
        .send(ControlSignal::Stop { ack: ack_tx })
        .await
        .expect("send stop");

    let outcome = tokio::time::timeout(Duration::from_secs(3), ack_rx)
        .await
        .expect("ack timeout")
        .expect("ack channel closed");

    assert!(matches!(outcome, StopOutcome::Killed { .. }));
    assert!(status_rx.borrow().is_terminal());
    monitor.await.expect("monitor task panic");
    drop(control_tx);
}

#[tokio::test]
async fn monitor_natural_exit_yields_completed_for_zero_code() {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg("exit 0").kill_on_drop(true);
    configure_process_group(&mut cmd);
    let child = cmd.spawn().unwrap();
    let pgid = capture_pgid(&child);

    let exit_code = Arc::new(AtomicI32::new(i32::MIN));
    let (status_tx, mut status_rx) = watch::channel(TaskStatus::Running);
    let (_control_tx, control_rx) = mpsc::channel(4);

    let monitor = tokio::spawn(run_process_monitor(
        child,
        pgid,
        vec![],
        exit_code.clone(),
        status_tx,
        control_rx,
        test_timing(),
    ));

    let wait = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if status_rx.changed().await.is_err() || status_rx.borrow().is_terminal() {
                return;
            }
        }
    });
    wait.await.expect("should terminate");
    assert_eq!(*status_rx.borrow(), TaskStatus::Completed);
    assert_eq!(exit_code.load(Ordering::Acquire), 0);
    monitor.await.expect("monitor task panic");
}
