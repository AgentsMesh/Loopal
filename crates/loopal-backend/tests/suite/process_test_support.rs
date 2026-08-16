#![cfg(unix)]

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) fn unique_pid_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "loopal-{label}-{}-{}.pid",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(crate) async fn wait_for_pid(path: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match tokio::fs::read_to_string(path).await {
            Ok(value) => {
                if let Ok(pid) = value.trim().parse::<i32>() {
                    assert!(pid > 1, "invalid process id in {}", path.display());
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
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub(crate) async fn wait_until_terminal(pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if !process_is_live(pid).expect("inspect process") {
            return;
        }
        assert!(Instant::now() < deadline, "process {pid} remained live");
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub(crate) async fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match tokio::fs::metadata(path).await {
            Ok(_) => return,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("failed inspecting {}: {error}", path.display()),
        }
        assert!(
            Instant::now() < deadline,
            "file not ready: {}",
            path.display()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub(crate) async fn remove_pid_file(path: &Path) {
    tokio::fs::remove_file(path).await.expect("remove pid file");
}

#[cfg(target_os = "linux")]
fn process_is_live(pid: i32) -> io::Result<bool> {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => {
            let state = stat
                .rsplit_once(") ")
                .and_then(|(_, fields)| fields.split_whitespace().next());
            Ok(!matches!(state, Some("Z" | "X")))
        }
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
    let error = io::Error::last_os_error();
    Ok(result == 0 || error.raw_os_error() != Some(libc::ESRCH))
}
