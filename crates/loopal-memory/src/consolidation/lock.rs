use std::path::Path;

use super::now_secs;

const LOCK_STALE_THRESHOLD_SECS: u64 = 3600;

pub fn try_acquire_lock(memory_dir: &Path) -> Option<std::path::PathBuf> {
    let lock_path = memory_dir.join(".consolidation_lock");
    let now = now_secs(); // capture once to avoid TOCTOU skew
    if lock_path.exists() {
        let is_stale = std::fs::read_to_string(&lock_path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|lock_ts| now.saturating_sub(lock_ts) > LOCK_STALE_THRESHOLD_SECS)
            .unwrap_or(true); // unparseable → treat as stale
        if !is_stale {
            return None; // another session owns it
        }
        tracing::info!("stale consolidation lock detected, overwriting");
    }
    let _ = std::fs::create_dir_all(memory_dir);
    match std::fs::write(&lock_path, now.to_string()) {
        Ok(()) => Some(lock_path),
        Err(e) => {
            tracing::warn!("failed to create consolidation lock: {e}");
            None
        }
    }
}

pub fn release_lock(lock_path: &Path) {
    let _ = std::fs::remove_file(lock_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_lock_fresh() {
        let dir = std::env::temp_dir().join("test_lock_fresh_v3");
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
        let dir = std::env::temp_dir().join("test_lock_held_v3");
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
        let dir = std::env::temp_dir().join("test_lock_stale_v3");
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
        let dir = std::env::temp_dir().join("test_lock_corrupted_v3");
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
        release_lock(Path::new("/tmp/nonexistent_lock_file_v3"));
    }
}
