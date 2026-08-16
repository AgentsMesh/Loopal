#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use loopal_backend::shell::{SpawnedBackgroundData, exec_background};
use loopal_tool_api::EnvOverride;
use loopal_tool_background::monitor::{MonitorTiming, run_process_monitor};
use loopal_tool_background::{ControlSignal, StopOutcome, TaskStatus};
use tokio::sync::{mpsc, oneshot, watch};

use crate::test_support::unique_sid;

async fn wait_for_pid(path: &std::path::Path) -> i32 {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(value) = tokio::fs::read_to_string(path).await
            && let Ok(pid) = value.trim().parse::<i32>()
        {
            return pid;
        }
        assert!(std::time::Instant::now() < deadline, "pid file not ready");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_until_terminal(pid: i32) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if !process_is_live(pid).expect("inspect process") {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "process {pid} remained live"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(target_os = "linux")]
fn process_is_live(pid: i32) -> std::io::Result<bool> {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => {
            let state = stat
                .rsplit_once(") ")
                .and_then(|(_, fields)| fields.split_whitespace().next());
            Ok(!matches!(state, Some("Z" | "X")))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn process_is_live(pid: i32) -> std::io::Result<bool> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size =
        i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).map_err(std::io::Error::other)?;
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            1,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read == size {
        return Ok(unsafe { info.assume_init() }.pbi_status != libc::SZOMB);
    }
    let result = unsafe { libc::kill(pid, 0) };
    let error = std::io::Error::last_os_error();
    Ok(result == 0 || error.raw_os_error() != Some(libc::ESRCH))
}

fn test_timing() -> MonitorTiming {
    MonitorTiming {
        drainers_grace: Duration::from_millis(200),
        sigterm_grace: Duration::from_millis(500),
    }
}

async fn spawn_process(command: &str) -> SpawnedBackgroundData {
    exec_background(
        &std::env::temp_dir(),
        None,
        command,
        &EnvOverride::default(),
        &unique_sid(),
    )
    .await
    .unwrap()
}

async fn monitor_for(
    data: SpawnedBackgroundData,
) -> (
    mpsc::Sender<ControlSignal>,
    watch::Receiver<TaskStatus>,
    Arc<AtomicI32>,
    tokio::task::JoinHandle<()>,
) {
    let exit_code = Arc::new(AtomicI32::new(i32::MIN));
    let (status_tx, status_rx) = watch::channel(TaskStatus::Running);
    let (control_tx, control_rx) = mpsc::channel(4);
    let handle = tokio::spawn(run_process_monitor(
        data.spawned,
        data.capture_task,
        data.capture_state,
        exit_code.clone(),
        status_tx,
        control_rx,
        test_timing(),
    ));
    (control_tx, status_rx, exit_code, handle)
}

async fn wait_terminal(status: &mut watch::Receiver<TaskStatus>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !status.borrow().is_terminal() {
            if status.changed().await.is_err() {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn monitor_force_kills_when_control_tx_dropped() {
    let data = spawn_process("sleep 30").await;
    let (control, mut status, _, monitor) = monitor_for(data).await;
    drop(control);
    wait_terminal(&mut status).await;
    assert_eq!(*status.borrow(), TaskStatus::Killed);
    monitor.await.unwrap();
}

#[tokio::test]
async fn monitor_returns_killed_outcome_to_stop_ack() {
    let data = spawn_process("sleep 30").await;
    let (control, status, _, monitor) = monitor_for(data).await;
    let (ack_tx, ack_rx) = oneshot::channel();
    control
        .send(ControlSignal::Stop { ack: ack_tx })
        .await
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(3), ack_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, StopOutcome::Killed { .. }));
    assert!(status.borrow().is_terminal());
    monitor.await.unwrap();
}

#[tokio::test]
async fn aborting_monitor_kills_descendant_via_owner_drop() {
    let pid_file = std::env::temp_dir().join(format!(
        "loopal-monitor-abort-{}.pid",
        uuid::Uuid::new_v4().simple()
    ));
    let command = format!(
        "(while :; do sleep 1; done) & child=$!; echo $child > '{}'; wait",
        pid_file.display()
    );
    let data = spawn_process(&command).await;
    let descendant = wait_for_pid(&pid_file).await;
    let (_control, _status, _, monitor) = monitor_for(data).await;

    monitor.abort();
    assert!(monitor.await.unwrap_err().is_cancelled());
    wait_until_terminal(descendant).await;
    tokio::fs::remove_file(pid_file).await.unwrap();
}

#[tokio::test]
async fn monitor_natural_exit_yields_completed_for_zero_code() {
    let data = spawn_process("exit 0").await;
    let (_control, mut status, exit_code, monitor) = monitor_for(data).await;
    wait_terminal(&mut status).await;
    assert_eq!(*status.borrow(), TaskStatus::Completed);
    assert_eq!(exit_code.load(Ordering::Acquire), 0);
    monitor.await.unwrap();
}
