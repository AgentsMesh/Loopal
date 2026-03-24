use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::locations::{global_plugins_dir, sessions_dir, tmp_dir};

/// Ensure volatile and persistent directories exist, then clean up expired files.
/// Called once at process startup; errors are silently ignored (best-effort).
///
/// Note: logs directory cleanup is handled by `log_writer::cleanup_old_logs`
/// in the binary crate, which applies both file-count and size limits.
pub fn startup_cleanup() {
    // Ensure volatile directories exist
    let _ = fs::create_dir_all(tmp_dir());
    // Ensure persistent directories exist
    if let Ok(d) = sessions_dir() {
        let _ = fs::create_dir_all(&d);
    }
    if let Ok(d) = global_plugins_dir() {
        let _ = fs::create_dir_all(&d);
    }
    // Clean up expired temp files
    cleanup_expired_files(&tmp_dir(), 1);
}

/// Remove files older than `max_age_days` from `dir` (non-recursive, best-effort).
fn cleanup_expired_files(dir: &Path, max_age_days: u64) {
    let cutoff = SystemTime::now() - Duration::from_secs(max_age_days * 24 * 60 * 60);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(meta) = path.metadata()
            && let Ok(modified) = meta.modified()
            && modified < cutoff
        {
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cleanup_expired_files_removes_old_and_keeps_new() {
        let dir = tempfile::tempdir().unwrap();
        let old_file = dir.path().join("old.log");
        let new_file = dir.path().join("new.log");

        fs::write(&old_file, "old").unwrap();
        fs::write(&new_file, "new").unwrap();

        // Backdate the old file by 10 days
        let ten_days_ago = SystemTime::now() - Duration::from_secs(10 * 86400);
        filetime::set_file_mtime(
            &old_file,
            filetime::FileTime::from_system_time(ten_days_ago),
        )
        .unwrap();

        cleanup_expired_files(dir.path(), 7);

        assert!(!old_file.exists(), "old file should be removed");
        assert!(new_file.exists(), "new file should be kept");
    }

    #[test]
    fn test_cleanup_expired_files_ignores_missing_dir() {
        let missing = Path::new("/tmp/loopal_test_nonexistent_dir_12345");
        // Should not panic
        cleanup_expired_files(missing, 1);
    }
}
