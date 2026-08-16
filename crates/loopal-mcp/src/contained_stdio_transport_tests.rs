#![cfg(unix)]

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rmcp::transport::Transport;
use tokio::process::Command;

use super::ContainedStdioTransport;

const POLL: Duration = Duration::from_millis(10);

#[tokio::test]
async fn dropping_transport_kills_direct_child() {
    let root_file = unique_path("mcp-stdio-root");
    let mut command = Command::new("sh");
    command
        .args(["-c", "echo $$ > \"$ROOT_FILE\"; while :; do sleep 1; done"])
        .env("ROOT_FILE", &root_file);
    let (transport, _) = ContainedStdioTransport::spawn(command).unwrap();
    let root = wait_for_pid(&root_file).await;

    drop(transport);

    wait_until_terminal(root).await;
    tokio::fs::remove_file(root_file).await.unwrap();
}

#[tokio::test]
async fn dropping_transport_kills_descendant() {
    let child_file = unique_path("mcp-stdio-descendant");
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            "(trap '' TERM; while :; do sleep 1; done) & echo $! > \"$CHILD_FILE\"; wait",
        ])
        .env("CHILD_FILE", &child_file);
    let (transport, _) = ContainedStdioTransport::spawn(command).unwrap();
    let descendant = wait_for_pid(&child_file).await;

    drop(transport);

    wait_until_terminal(descendant).await;
    tokio::fs::remove_file(child_file).await.unwrap();
}

#[tokio::test]
async fn close_waits_for_eof_aware_child_and_is_idempotent() {
    let mut command = Command::new("sh");
    command.args(["-c", "while read line; do :; done"]);
    let (mut transport, stderr) = ContainedStdioTransport::spawn(command).unwrap();
    assert!(stderr.is_some());

    transport.close().await.unwrap();
    transport.close().await.unwrap();
}

#[tokio::test]
async fn close_terminates_child_that_ignores_eof() {
    let mut command = Command::new("sh");
    command.args(["-c", "trap '' TERM; exec sleep 30"]);
    let (mut transport, _) = ContainedStdioTransport::spawn(command).unwrap();

    transport.close().await.unwrap();
}

fn unique_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "loopal-{label}-{}-{}.pid",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

async fn wait_for_pid(path: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match tokio::fs::read_to_string(path).await {
            Ok(value) => {
                if let Ok(pid) = value.trim().parse::<i32>() {
                    return pid;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("failed reading {}: {error}", path.display()),
        }
        assert!(Instant::now() < deadline, "pid file was not written");
        tokio::time::sleep(POLL).await;
    }
}

async fn wait_until_terminal(pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while process_is_live(pid).unwrap() {
        assert!(Instant::now() < deadline, "process {pid} remained live");
        tokio::time::sleep(POLL).await;
    }
}

#[cfg(target_os = "linux")]
fn process_is_live(pid: i32) -> io::Result<bool> {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => Ok(!matches!(
            stat.rsplit_once(") ")
                .and_then(|(_, fields)| fields.split_whitespace().next()),
            Some("Z" | "X")
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn process_is_live(pid: i32) -> io::Result<bool> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size =
        i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).map_err(io::Error::other)?;
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
    Ok(result == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH))
}
