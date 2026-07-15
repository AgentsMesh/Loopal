use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use loopal_error::LoopalError;

pub(super) struct SettingsFileLock {
    #[cfg(unix)]
    file: File,
    #[cfg(not(unix))]
    path: PathBuf,
    #[cfg(not(unix))]
    owner: String,
}

impl SettingsFileLock {
    #[cfg(unix)]
    pub(super) fn acquire(path: PathBuf) -> Result<Self, LoopalError> {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .map_err(LoopalError::Io)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(LoopalError::Io)?;
        for _ in 0..500 {
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result == 0 {
                return Ok(Self { file });
            }
            let error = std::io::Error::last_os_error();
            if error
                .raw_os_error()
                .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
            {
                std::thread::sleep(Duration::from_millis(10));
            } else {
                return Err(LoopalError::Io(error));
            }
        }
        Err(timeout())
    }

    #[cfg(not(unix))]
    pub(super) fn acquire(path: PathBuf) -> Result<Self, LoopalError> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_LOCK: AtomicU64 = AtomicU64::new(1);
        let owner = format!(
            "{}:{}",
            std::process::id(),
            NEXT_LOCK.fetch_add(1, Ordering::Relaxed)
        );
        for _ in 0..500 {
            match super::atomic_settings_write::create_private(&path, 0o600) {
                Ok(mut file) => {
                    file.write_all(owner.as_bytes()).map_err(LoopalError::Io)?;
                    file.sync_all().map_err(LoopalError::Io)?;
                    return Ok(Self { path, owner });
                }
                Err(LoopalError::Io(error)) if error.kind() == ErrorKind::AlreadyExists => {
                    reclaim_stale(&path);
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
        Err(timeout())
    }
}

impl Drop for SettingsFileLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
        #[cfg(not(unix))]
        if std::fs::read_to_string(&self.path).as_deref() == Ok(&self.owner) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn timeout() -> LoopalError {
    LoopalError::Io(std::io::Error::new(
        ErrorKind::TimedOut,
        "timed out waiting for settings writer",
    ))
}

#[cfg(not(unix))]
fn reclaim_stale(path: &PathBuf) {
    let observed = std::fs::read_to_string(path).ok();
    let stale = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > Duration::from_secs(4));
    if stale && observed.is_some() && std::fs::read_to_string(path).ok() == observed {
        let _ = std::fs::remove_file(path);
    }
}
