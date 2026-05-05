use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

static CLEANED: AtomicBool = AtomicBool::new(false);

const TTL: Duration = Duration::from_secs(24 * 3600);

pub fn cleanup_old_files_once(tmp_dir: &Path) {
    if CLEANED.swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(cutoff) = SystemTime::now().checked_sub(TTL) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(tmp_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let too_old = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| t < cutoff)
            .unwrap_or(false);
        if too_old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
pub fn reset_for_test() {
    CLEANED.store(false, Ordering::Relaxed);
}
