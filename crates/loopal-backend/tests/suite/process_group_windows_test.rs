#![cfg(windows)]

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use loopal_backend::{KillOutcome, SpawnedChild};
use tokio::process::Command;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, STILL_ACTIVE};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

const POLL: Duration = Duration::from_millis(20);

fn descendant_command(pid_file: &Path, leader_exits: bool) -> Command {
    let script = r#"
$child = Start-Process -PassThru -WindowStyle Hidden -FilePath 'powershell.exe' -ArgumentList '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 30"'
[System.IO.File]::WriteAllText($env:PID_FILE, $child.Id.ToString())
if ($env:LEADER_EXITS -eq '1') { exit 0 }
Wait-Process -Id $child.Id
"#;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("PID_FILE", pid_file)
        .env("LEADER_EXITS", if leader_exits { "1" } else { "0" })
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[tokio::test]
async fn job_terminate_proves_descendant_terminal() {
    let path = unique_pid_path("terminate-job");
    let mut spawned = SpawnedChild::spawn(descendant_command(&path, false)).unwrap();
    let descendant = wait_for_pid(&path).await;

    let termination = spawned.terminate(Duration::from_secs(5)).await;
    assert_eq!(termination.outcome, KillOutcome::Killed);
    wait_until_terminal(descendant).await;
    remove_pid_file(&path).await;
}

#[tokio::test]
async fn root_exit_terminates_residual_job_member() {
    let path = unique_pid_path("root-exit-job");
    let mut spawned = SpawnedChild::spawn(descendant_command(&path, true)).unwrap();
    let descendant = wait_for_pid(&path).await;

    let status = tokio::time::timeout(Duration::from_secs(10), spawned.wait())
        .await
        .expect("job wait bounded")
        .expect("job wait succeeds");
    assert!(status.success());
    wait_until_terminal(descendant).await;
    remove_pid_file(&path).await;
}

#[tokio::test]
async fn dropping_armed_job_owner_kills_descendant() {
    let path = unique_pid_path("drop-job");
    let spawned = SpawnedChild::spawn(descendant_command(&path, false)).unwrap();
    let descendant = wait_for_pid(&path).await;

    drop(spawned);
    wait_until_terminal(descendant).await;
    remove_pid_file(&path).await;
}

fn unique_pid_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "loopal-{label}-{}-{}.pid",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

async fn wait_for_pid(path: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match tokio::fs::read_to_string(path).await {
            Ok(value) => {
                if let Ok(pid) = value.trim().parse::<u32>() {
                    return pid;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("failed reading {}: {error}", path.display()),
        }
        assert!(
            Instant::now() < deadline,
            "pid file not ready: {}",
            path.display()
        );
        tokio::time::sleep(POLL).await;
    }
}

async fn wait_until_terminal(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while process_is_live(pid).expect("inspect child process") {
        assert!(Instant::now() < deadline, "process {pid} remained live");
        tokio::time::sleep(POLL).await;
    }
}

fn process_is_live(pid: u32) -> io::Result<bool> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle == 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(false)
        } else {
            Err(error)
        };
    }
    let mut exit_code = 0u32;
    let read = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
    let close = unsafe { CloseHandle(handle) };
    if read == 0 || close == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(exit_code == STILL_ACTIVE as u32)
}

async fn remove_pid_file(path: &Path) {
    tokio::fs::remove_file(path).await.expect("remove pid file");
}
