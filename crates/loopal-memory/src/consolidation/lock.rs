use std::io::Write;
use std::path::Path;

use super::now_secs;

const LOCK_STALE_THRESHOLD_SECS: u64 = 3600;
const LOCK_INCOMPLETE_GRACE_SECS: u64 = 30;

pub fn try_acquire_lock(memory_dir: &Path) -> Option<std::path::PathBuf> {
    let lock_path = memory_dir.join(".consolidation_lock");
    let now = now_secs();
    let _ = std::fs::create_dir_all(memory_dir);

    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut f) => {
                if let Err(e) = writeln!(f, "{}", now) {
                    tracing::warn!("failed to write consolidation lock body: {e}");
                    let _ = std::fs::remove_file(&lock_path);
                    return None;
                }
                return Some(lock_path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let raw = std::fs::read_to_string(&lock_path).ok();
                let is_stale = match raw.as_deref().map(str::trim) {
                    None => true,
                    Some("") => incomplete_lock_is_stale(&lock_path, now),
                    Some(s) => match s.parse::<u64>() {
                        Ok(lock_ts) => now.saturating_sub(lock_ts) > LOCK_STALE_THRESHOLD_SECS,
                        Err(_) => true,
                    },
                };
                if !is_stale {
                    return None;
                }
                tracing::info!("stale consolidation lock detected, replacing");
                if std::fs::remove_file(&lock_path).is_err() {
                    return None;
                }
            }
            Err(e) => {
                tracing::warn!("failed to create consolidation lock: {e}");
                return None;
            }
        }
    }
}

fn incomplete_lock_is_stale(lock_path: &Path, now: u64) -> bool {
    let mtime = std::fs::metadata(lock_path).and_then(|m| m.modified()).ok();
    let Some(mtime) = mtime else {
        return true;
    };
    let mtime_secs = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(mtime_secs) > LOCK_INCOMPLETE_GRACE_SECS
}

pub fn release_lock(lock_path: &Path) {
    let _ = std::fs::remove_file(lock_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_lock_fresh() {
        let dir = std::env::temp_dir().join("test_lock_fresh_v4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let lock = try_acquire_lock(&dir);
        assert!(lock.is_some(), "should acquire lock on empty dir");
        assert!(dir.join(".consolidation_lock").exists());

        release_lock(&lock.unwrap());
        assert!(!dir.join(".consolidation_lock").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_acquire_lock_already_held() {
        let dir = std::env::temp_dir().join("test_lock_held_v4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let lock1 = try_acquire_lock(&dir);
        assert!(lock1.is_some());

        let lock2 = try_acquire_lock(&dir);
        assert!(lock2.is_none(), "should not acquire when lock is held");

        release_lock(&lock1.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_acquire_lock_stale() {
        let dir = std::env::temp_dir().join("test_lock_stale_v4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let old_ts = now_secs() - 7200;
        std::fs::write(dir.join(".consolidation_lock"), old_ts.to_string()).unwrap();

        let lock = try_acquire_lock(&dir);
        assert!(lock.is_some(), "should overwrite stale lock");

        release_lock(&lock.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_acquire_lock_corrupted() {
        let dir = std::env::temp_dir().join("test_lock_corrupted_v4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join(".consolidation_lock"), "garbage").unwrap();

        let lock = try_acquire_lock(&dir);
        assert!(lock.is_some(), "should overwrite corrupted lock");

        release_lock(&lock.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_release_lock_nonexistent() {
        release_lock(Path::new("/tmp/nonexistent_lock_file_v4"));
    }

    #[test]
    fn test_acquire_lock_empty_within_grace_period() {
        let dir = std::env::temp_dir().join("test_lock_empty_fresh_v4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".consolidation_lock"), "").unwrap();

        let lock = try_acquire_lock(&dir);
        assert!(
            lock.is_none(),
            "fresh empty lock should be treated as winner mid-write"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_acquire_lock_empty_past_grace_period() {
        let dir = std::env::temp_dir().join("test_lock_empty_stale_v4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lock_path = dir.join(".consolidation_lock");
        std::fs::write(&lock_path, "").unwrap();

        let backdated = std::time::SystemTime::now()
            - std::time::Duration::from_secs(LOCK_INCOMPLETE_GRACE_SECS + 60);
        let f = std::fs::File::options()
            .write(true)
            .open(&lock_path)
            .unwrap();
        f.set_modified(backdated).unwrap();

        let lock = try_acquire_lock(&dir);
        assert!(
            lock.is_some(),
            "empty lock older than grace period should be reclaimable"
        );

        release_lock(&lock.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
