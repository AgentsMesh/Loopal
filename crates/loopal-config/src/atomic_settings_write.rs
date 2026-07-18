use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use loopal_error::{ConfigError, LoopalError};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

pub(super) fn atomic_write(path: &Path, data: &[u8]) -> Result<(), LoopalError> {
    let parent = path.parent().ok_or_else(|| ConfigError::InvalidValue {
        field: path.display().to_string(),
        reason: "path has no parent directory".into(),
    })?;
    let tmp = parent.join(format!(
        ".{}.tmp.{}.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("settings.local.json"),
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
    ));
    {
        let mut file = create_private(&tmp, existing_private_mode(path)?)?;
        file.write_all(data).map_err(LoopalError::Io)?;
        file.sync_all().map_err(LoopalError::Io)?;
    }
    rename_with_windows_retry(&tmp, path).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        LoopalError::Io(error)
    })?;
    sync_parent(parent)
}

#[cfg(unix)]
fn rename_with_windows_retry(tmp: &Path, path: &Path) -> std::io::Result<()> {
    std::fs::rename(tmp, path)
}

// reason: on Windows a rename target briefly held open (antivirus, indexer,
// a reader mid-close) fails with ACCESS_DENIED even though it clears within
// milliseconds; a bounded retry turns that transient into success instead of
// surfacing a spurious write failure.
#[cfg(not(unix))]
fn rename_with_windows_retry(tmp: &Path, path: &Path) -> std::io::Result<()> {
    let mut last = None;
    for _ in 0..50 {
        match std::fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                last = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::from(std::io::ErrorKind::PermissionDenied)))
}

pub(super) fn create_private(path: &Path, mode: u32) -> Result<File, LoopalError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    options.open(path).map_err(LoopalError::Io)
}

fn existing_private_mode(_path: &Path) -> Result<u32, LoopalError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(_path) {
            Ok(metadata) => {
                let mode = metadata.permissions().mode() & 0o600;
                Ok(if mode == 0 { 0o600 } else { mode })
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(0o600),
            Err(error) => Err(LoopalError::Io(error)),
        }
    }
    #[cfg(not(unix))]
    Ok(0o600)
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<(), LoopalError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(LoopalError::Io)
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<(), LoopalError> {
    Ok(())
}
