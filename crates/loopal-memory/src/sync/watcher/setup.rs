use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loopal_error::MemoryGraphError;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer, notify::RecursiveMode};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::store::MemoryGraph;
use crate::sync::watcher::process_events;

const DEBOUNCE_MS: u64 = 500;

pub struct WatcherHandle {
    _debouncer: Mutex<Box<dyn std::any::Any + Send>>,
    pub task: JoinHandle<()>,
}

pub fn watch(graph: Arc<MemoryGraph>, dir: PathBuf) -> Result<WatcherHandle, MemoryGraphError> {
    let (tx, rx) = mpsc::unbounded_channel::<Vec<PathBuf>>();
    let mut debouncer = new_debouncer(
        Duration::from_millis(DEBOUNCE_MS),
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                let paths: Vec<PathBuf> = events
                    .into_iter()
                    .filter(|e| {
                        e.path
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s == "md")
                            .unwrap_or(false)
                    })
                    .map(|e| e.path)
                    .collect();
                if !paths.is_empty() {
                    let _ = tx.send(paths);
                }
            }
            Err(err) => {
                warn!(error = %err, "watcher error");
            }
        },
    )
    .map_err(|e| MemoryGraphError::Watcher(format!("init: {e}")))?;

    debouncer
        .watcher()
        .watch(&dir, RecursiveMode::Recursive)
        .map_err(|e| MemoryGraphError::Watcher(format!("watch: {e}")))?;

    let base = dir.clone();
    let task = tokio::spawn(async move { process_events(graph, base, rx).await });

    Ok(WatcherHandle {
        _debouncer: Mutex::new(Box::new(debouncer)),
        task,
    })
}
