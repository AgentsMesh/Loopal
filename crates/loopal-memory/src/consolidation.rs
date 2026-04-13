//! Memory consolidation — scheduling, locking, and TTL utilities.
//!
//! Filesystem I/O is intentionally kept thin (marker file read/write only).
//! Heavy operations (agent spawning, prompt building) live in loopal-agent-server.

use std::path::Path;

use crate::date;

/// Maximum age of a consolidation lock before it's considered stale (seconds).
const LOCK_STALE_THRESHOLD_SECS: u64 = 3600; // 1 hour

/// Try to acquire the consolidation lock. Returns the lock file path on success,
/// or `None` if another session holds a fresh lock.
///
/// The lock file contains a Unix timestamp. Stale locks (> 1 hour) are overwritten.
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
    // Ensure directory exists
    let _ = std::fs::create_dir_all(memory_dir);
    match std::fs::write(&lock_path, now.to_string()) {
        Ok(()) => Some(lock_path),
        Err(e) => {
            tracing::warn!("failed to create consolidation lock: {e}");
            None
        }
    }
}

/// Release the consolidation lock.
pub fn release_lock(lock_path: &Path) {
    let _ = std::fs::remove_file(lock_path);
}

/// Check whether memory consolidation is due based on the `.last_consolidation` marker file.
pub fn needs_consolidation(memory_dir: &Path, interval_days: u32) -> bool {
    let marker = memory_dir.join(".last_consolidation");
    match std::fs::read_to_string(&marker) {
        Ok(content) => {
            let last = content.trim();
            let today = date::today_str();
            date::days_between(last, &today).is_none_or(|d| d >= interval_days as i64)
        }
        Err(_) => {
            // No marker file — consolidation never ran, but only trigger if memory dir exists
            memory_dir.join("MEMORY.md").exists()
        }
    }
}

/// Mark consolidation as done by writing today's date to the marker file.
pub fn mark_done(memory_dir: &Path) {
    // Ensure directory exists before writing marker.
    if let Err(e) = std::fs::create_dir_all(memory_dir) {
        tracing::warn!("failed to create memory dir for consolidation marker: {e}");
        return;
    }
    let marker = memory_dir.join(".last_consolidation");
    if let Err(e) = std::fs::write(&marker, date::today_str()) {
        tracing::warn!("failed to write consolidation marker: {e}");
    }
}

/// Check whether a topic file has expired based on its frontmatter metadata.
///
/// Pure function — no I/O. Caller provides the parsed values.
pub fn is_expired(created_at: &str, ttl_days: Option<u32>) -> bool {
    match ttl_days {
        None => false, // never expires
        Some(ttl) => {
            let today = date::today_str();
            date::days_between(created_at, &today).is_some_and(|d| d >= ttl as i64)
        }
    }
}

/// Current Unix timestamp in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Consolidation scheduling tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_needs_consolidation_no_marker_no_memory() {
        let dir = std::env::temp_dir().join("test_consol_no_marker_no_mem_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!needs_consolidation(&dir, 7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_needs_consolidation_no_marker_with_memory() {
        let dir = std::env::temp_dir().join("test_consol_no_marker_with_mem_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMORY.md"), "# Memory\nSome content").unwrap();
        assert!(needs_consolidation(&dir, 7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_needs_consolidation_recent_marker() {
        let dir = std::env::temp_dir().join("test_consol_recent_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMORY.md"), "# Memory").unwrap();
        std::fs::write(dir.join(".last_consolidation"), date::today_str()).unwrap();
        assert!(!needs_consolidation(&dir, 7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_needs_consolidation_overdue_marker() {
        let dir = std::env::temp_dir().join("test_consol_overdue_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMORY.md"), "# Memory").unwrap();
        let old_days = now_secs() / 86400 - 10;
        let old_date = date::epoch_days_to_date(old_days as i64);
        std::fs::write(dir.join(".last_consolidation"), &old_date).unwrap();
        assert!(needs_consolidation(&dir, 7));
        assert!(!needs_consolidation(&dir, 30));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_needs_consolidation_corrupted_marker() {
        let dir = std::env::temp_dir().join("test_consol_corrupted_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMORY.md"), "# Memory").unwrap();
        std::fs::write(dir.join(".last_consolidation"), "not-a-date").unwrap();
        assert!(needs_consolidation(&dir, 7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------------
    // mark_done tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mark_done_writes_today() {
        let dir = std::env::temp_dir().join("test_consol_mark_done_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        mark_done(&dir);
        let content = std::fs::read_to_string(dir.join(".last_consolidation")).unwrap();
        assert_eq!(content.trim(), date::today_str());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_mark_done_overwrites_existing() {
        let dir = std::env::temp_dir().join("test_consol_mark_overwrite_v3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".last_consolidation"), "2020-01-01").unwrap();
        mark_done(&dir);
        let content = std::fs::read_to_string(dir.join(".last_consolidation")).unwrap();
        assert_eq!(content.trim(), date::today_str());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_mark_done_creates_directory() {
        let dir = std::env::temp_dir().join("test_consol_mark_creates_dir_v3");
        let _ = std::fs::remove_dir_all(&dir);
        // Directory does NOT exist — mark_done should create it
        mark_done(&dir);
        assert!(dir.join(".last_consolidation").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------------
    // TTL / expiration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_expired_no_ttl() {
        assert!(!is_expired("2020-01-01", None));
    }

    #[test]
    fn test_is_expired_within_ttl() {
        let today = date::today_str();
        assert!(!is_expired(&today, Some(90)));
    }

    #[test]
    fn test_is_expired_past_ttl() {
        assert!(is_expired("2020-01-01", Some(90)));
    }

    #[test]
    fn test_is_expired_future_date() {
        // Future created_at: days_between returns negative → not expired
        assert!(!is_expired("2099-01-01", Some(90)));
    }

    // -----------------------------------------------------------------------
    // Lock tests
    // -----------------------------------------------------------------------

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

        // Second acquire fails (lock is fresh)
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

        // Write a lock from 2 hours ago (stale)
        let old_ts = now_secs() - 7200;
        std::fs::write(dir.join(".consolidation_lock"), old_ts.to_string()).unwrap();

        // Should overwrite stale lock
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

        // Corrupted lock file (not a valid timestamp)
        std::fs::write(dir.join(".consolidation_lock"), "garbage").unwrap();

        // Should treat as stale and overwrite
        let lock = try_acquire_lock(&dir);
        assert!(lock.is_some(), "should overwrite corrupted lock");

        release_lock(&lock.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_release_lock_nonexistent() {
        // Should not panic
        release_lock(Path::new("/tmp/nonexistent_lock_file_v3"));
    }
}
