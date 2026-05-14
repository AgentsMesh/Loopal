mod lock;
mod schedule;

pub use lock::{release_lock, try_acquire_lock};
pub use schedule::{is_expired, mark_done, needs_consolidation};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
