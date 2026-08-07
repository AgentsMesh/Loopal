use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::notification::{
    ServiceNotification, publish_file_changed, publish_git_changed, publish_resync_required,
};
use crate::{RootGuard, WorkspaceError};

const QUIET_PERIOD: Duration = Duration::from_millis(75);
const MAX_BATCH_AGE: Duration = Duration::from_millis(250);
const MAX_FILE_EVENTS: usize = 256;
const CHANNEL_CAPACITY: usize = 1_024;

pub(crate) struct WatcherHandle {
    watcher: Option<RecommendedWatcher>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        drop(self.watcher.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
struct Batch {
    files: BTreeMap<String, &'static str>,
    git_changed: bool,
    resync_reason: Option<&'static str>,
}

enum WatchInput {
    Event(Event),
    Resync,
}

pub(crate) fn start(
    guard: RootGuard,
    workspace_id: String,
    events: broadcast::Sender<ServiceNotification>,
) -> Result<WatcherHandle, WorkspaceError> {
    let root = guard.root().to_path_buf();
    let (tx, rx) = std::sync::mpsc::sync_channel(CHANNEL_CAPACITY);
    let dropped = Arc::new(AtomicBool::new(false));
    let callback_dropped = dropped.clone();
    let mut watcher = notify::recommended_watcher(move |result| match result {
        Ok(event) => enqueue(&tx, &callback_dropped, WatchInput::Event(event)),
        Err(error) => {
            tracing::warn!(error = %error, "workspace watcher failed");
            enqueue(&tx, &callback_dropped, WatchInput::Resync);
        }
    })
    .map_err(WorkspaceError::io)?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(WorkspaceError::io)?;
    let worker = std::thread::Builder::new()
        .name("loopal-workspace-watch".into())
        .spawn(move || run(guard, workspace_id, events, rx, dropped))
        .map_err(WorkspaceError::io)?;
    Ok(WatcherHandle {
        watcher: Some(watcher),
        worker: Some(worker),
    })
}

fn run(
    guard: RootGuard,
    workspace_id: String,
    events: broadcast::Sender<ServiceNotification>,
    rx: Receiver<WatchInput>,
    dropped: Arc<AtomicBool>,
) {
    while let Ok(first) = rx.recv() {
        let started = Instant::now();
        let mut batch = Batch::default();
        batch.add(&guard, first);
        loop {
            let remaining = MAX_BATCH_AGE.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(QUIET_PERIOD.min(remaining)) {
                Ok(event) => batch.add(&guard, event),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    batch.mark_dropped(&dropped);
                    batch.publish(&workspace_id, &events);
                    return;
                }
            }
        }
        batch.mark_dropped(&dropped);
        batch.publish(&workspace_id, &events);
    }
}

fn enqueue(tx: &SyncSender<WatchInput>, dropped: &AtomicBool, input: WatchInput) {
    match tx.try_send(input) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => dropped.store(true, Ordering::Release),
        Err(TrySendError::Disconnected(_)) => {}
    }
}

impl Batch {
    fn add(&mut self, guard: &RootGuard, input: WatchInput) {
        let WatchInput::Event(event) = input else {
            self.resync_reason = Some("watcher_error");
            return;
        };
        let Some(kind) = event_kind(&event.kind) else {
            return;
        };
        for path in event.paths {
            let Ok(relative) = guard.relative(&path) else {
                continue;
            };
            if relative.ends_with(".loopal.tmp") {
                continue;
            }
            if relative.split('/').any(|part| part == ".git") {
                continue;
            }
            self.git_changed = true;
            if self.files.len() < MAX_FILE_EVENTS || self.files.contains_key(&relative) {
                self.files.insert(relative, kind);
            } else {
                self.resync_reason.get_or_insert("watcher_overflow");
            }
        }
    }

    fn mark_dropped(&mut self, dropped: &AtomicBool) {
        if dropped.swap(false, Ordering::AcqRel) {
            self.resync_reason.get_or_insert("watcher_overflow");
        }
    }

    fn publish(self, workspace_id: &str, events: &broadcast::Sender<ServiceNotification>) {
        for (path, kind) in self.files {
            publish_file_changed(events, workspace_id, &path, kind);
        }
        if self.git_changed {
            publish_git_changed(events, workspace_id);
        }
        if let Some(reason) = self.resync_reason {
            tracing::warn!(reason, "workspace watcher requires resync");
            publish_resync_required(events, workspace_id, reason);
        }
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;

fn event_kind(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Create(_) => Some("created"),
        EventKind::Modify(_) => Some("changed"),
        EventKind::Remove(_) => Some("deleted"),
        _ => None,
    }
}
