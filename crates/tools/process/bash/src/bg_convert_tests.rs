use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loopal_error::ProcessHandle;

use crate::bg_convert::{register, register_spawned};

struct DropSentinel(Arc<AtomicBool>);

impl Drop for DropSentinel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn wrong_timeout_handle_is_rejected_and_dropped() {
    let dropped = Arc::new(AtomicBool::new(false));
    let handle = ProcessHandle(Box::new(DropSentinel(dropped.clone())));
    let store = loopal_tool_background::BackgroundTaskStore::new();

    let error = register(&store, handle, "command").unwrap_err();
    assert_eq!(error, "timed-out process adoption failed");
    assert!(dropped.load(Ordering::Acquire));
}

#[test]
fn wrong_background_handle_is_rejected_and_dropped() {
    let dropped = Arc::new(AtomicBool::new(false));
    let handle = ProcessHandle(Box::new(DropSentinel(dropped.clone())));
    let store = loopal_tool_background::BackgroundTaskStore::new();

    let error = register_spawned(&store, handle, "description").unwrap_err();
    assert_eq!(error, "background process adoption failed");
    assert!(dropped.load(Ordering::Acquire));
}
