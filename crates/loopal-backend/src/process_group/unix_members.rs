use std::io;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) async fn has_live(pgid: i32) -> io::Result<bool> {
    tokio::task::spawn_blocking(move || has_live_members(pgid))
        .await
        .map_err(|_| io::Error::other("process group inspection task failed"))?
}

pub(super) async fn wait_until_terminal(pgid: i32, timeout: Option<Duration>) -> io::Result<()> {
    let deadline = timeout.map(|duration| Instant::now() + duration);
    loop {
        if !has_live(pgid).await? {
            return Ok(());
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "process group terminal proof timed out",
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[cfg(target_os = "linux")]
fn has_live_members(pgid: i32) -> io::Result<bool> {
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some(fields) = stat.rsplit_once(") ").map(|(_, fields)| fields) else {
            continue;
        };
        let mut fields = fields.split_whitespace();
        let state = fields.next();
        let _parent = fields.next();
        let group = fields.next().and_then(|value| value.parse::<i32>().ok());
        if group == Some(pgid) && !matches!(state, Some("Z" | "X")) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(target_os = "macos")]
fn has_live_members(pgid: i32) -> io::Result<bool> {
    let mut capacity = 64usize;
    loop {
        let mut pids = vec![0i32; capacity];
        let bytes = capacity
            .checked_mul(std::mem::size_of::<i32>())
            .and_then(|size| i32::try_from(size).ok())
            .ok_or_else(|| io::Error::other("process group is too large to inspect"))?;
        unsafe { *libc::__error() = 0 };
        let count = unsafe { libc::proc_listpgrppids(pgid, pids.as_mut_ptr().cast(), bytes) };
        let error = io::Error::last_os_error();
        if count < 0 || (count == 0 && error.raw_os_error().is_some_and(|code| code != 0)) {
            return Err(error);
        }
        let count = usize::try_from(count).unwrap_or(0);
        if count >= capacity {
            capacity = next_capacity(capacity)?;
            continue;
        }
        return pids[..count]
            .iter()
            .filter(|pid| **pid > 0)
            .try_fold(false, |live, pid| Ok(live || process_is_live(*pid)?));
    }
}

#[cfg(target_os = "macos")]
fn next_capacity(capacity: usize) -> io::Result<usize> {
    capacity
        .checked_mul(2)
        .filter(|next| *next <= 65_536)
        .ok_or_else(|| io::Error::other("process group member limit exceeded"))
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
    if unsafe { libc::kill(pid, 0) } == 0 {
        Ok(true)
    } else if io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        Ok(false)
    } else {
        Ok(true)
    }
}

#[cfg(test)]
#[path = "unix_members_tests.rs"]
mod tests;
