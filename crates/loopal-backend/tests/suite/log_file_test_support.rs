use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn unique_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
