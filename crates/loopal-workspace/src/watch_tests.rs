use std::sync::atomic::{AtomicBool, Ordering};

use notify::event::ModifyKind;
use notify::{Event, EventKind};

use super::{Batch, WatchInput, enqueue};
use crate::RootGuard;

#[test]
fn bounded_input_marks_overflow_without_blocking() {
    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
    let dropped = AtomicBool::new(false);
    enqueue(&tx, &dropped, WatchInput::Event(event("first")));
    enqueue(&tx, &dropped, WatchInput::Event(event("second")));
    assert!(dropped.load(Ordering::Acquire));
}

#[test]
fn batch_deduplicates_paths_and_requests_resync_on_overflow() {
    let root = tempfile::tempdir().unwrap();
    let guard = RootGuard::new(root.path()).unwrap();
    let path = guard.root().join("src.rs");
    let mut batch = Batch::default();
    batch.add(&guard, WatchInput::Event(event(path.clone())));
    batch.add(&guard, WatchInput::Event(event(path)));
    batch.add(&guard, WatchInput::Resync);
    let (tx, mut rx) = tokio::sync::broadcast::channel(8);
    batch.publish("local-workspace", &tx);
    let notifications: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let methods: Vec<_> = notifications.iter().map(|item| item.method).collect();
    assert_eq!(
        methods,
        [
            "workspace/fileChanged",
            "workspace/gitChanged",
            "workspace/resyncRequired"
        ]
    );
    assert_eq!(notifications[2].params["reason"], "watcher_error");
}

fn event(path: impl Into<std::path::PathBuf>) -> Event {
    Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.into())
}
