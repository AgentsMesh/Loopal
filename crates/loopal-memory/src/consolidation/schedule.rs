use std::path::Path;

use crate::date;

pub fn needs_consolidation(memory_dir: &Path, interval_days: u32) -> bool {
    let marker = memory_dir.join(".last_consolidation");
    match std::fs::read_to_string(&marker) {
        Ok(content) => {
            let last = content.trim();
            let today = date::today_str();
            date::days_between(last, &today).is_none_or(|d| d >= interval_days as i64)
        }
        Err(_) => memory_dir.join("MEMORY.md").exists(),
    }
}

pub fn mark_done(memory_dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(memory_dir) {
        tracing::warn!("failed to create memory dir for consolidation marker: {e}");
        return;
    }
    let marker = memory_dir.join(".last_consolidation");
    if let Err(e) = std::fs::write(&marker, date::today_str()) {
        tracing::warn!("failed to write consolidation marker: {e}");
    }
}

pub fn is_expired(created_at: &str, ttl_days: Option<u32>) -> bool {
    match ttl_days {
        None => false,
        Some(ttl) => {
            let today = date::today_str();
            date::days_between(created_at, &today).is_some_and(|d| d >= ttl as i64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::now_secs;
    use super::*;

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
        mark_done(&dir);
        assert!(dir.join(".last_consolidation").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        assert!(!is_expired("2099-01-01", Some(90)));
    }
}
