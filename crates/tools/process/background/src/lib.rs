//! Background task store — owns running/completed background process state.
//!
//! `BackgroundTaskStore` is an injectable instance (not a global singleton)
//! so that tests can create isolated stores without cross-contamination.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::process::Child;
use tokio::sync::watch;

use loopal_protocol::{BgTaskSnapshot, BgTaskStatus};

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
}

pub struct BackgroundTask {
    pub output: Arc<Mutex<String>>,
    pub exit_code: Arc<Mutex<Option<i32>>>,
    pub status: Arc<Mutex<TaskStatus>>,
    pub description: String,
    pub child: Arc<Mutex<Option<Child>>>,
    /// Watch channel for event-driven status notification.
    pub status_watch: watch::Receiver<TaskStatus>,
}

/// Injectable background task store.
///
/// In production a single `Arc<BackgroundTaskStore>` is created at startup
/// and shared by `BashTool` + TUI.  In tests each test creates its own
/// instance — no global state, no `clear_store()`, no serialization mutex.
pub struct BackgroundTaskStore {
    tasks: Mutex<HashMap<String, BackgroundTask>>,
    counter: AtomicU64,
}

impl BackgroundTaskStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            tasks: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
        })
    }

    pub fn generate_task_id(&self) -> String {
        format!("bg_{}", self.counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn insert(&self, id: String, task: BackgroundTask) {
        self.tasks.lock().unwrap().insert(id, task);
    }

    /// Access a task by ID under the store lock.
    ///
    /// The closure runs while the lock is held — keep it short.
    pub fn with_task<T>(&self, id: &str, f: impl FnOnce(&BackgroundTask) -> T) -> Option<T> {
        let guard = self.tasks.lock().unwrap();
        guard.get(id).map(f)
    }

    /// Snapshot all running background tasks as protocol-level types.
    pub fn snapshot_running(&self) -> Vec<BgTaskSnapshot> {
        let guard = self.tasks.lock().unwrap();
        let mut out: Vec<BgTaskSnapshot> = guard
            .iter()
            .filter_map(|(id, task)| {
                let status = task.status.lock().unwrap().clone();
                if status != TaskStatus::Running {
                    return None;
                }
                Some(BgTaskSnapshot {
                    id: id.clone(),
                    description: task.description.clone(),
                    status: BgTaskStatus::Running,
                })
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Register a proxy task for a background agent process.
    pub fn register_proxy(&self, id: String, description: String) -> ProxyHandle {
        let output = Arc::new(Mutex::new(String::new()));
        let exit_code = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(TaskStatus::Running));
        let (watch_tx, watch_rx) = watch::channel(TaskStatus::Running);
        let handle = ProxyHandle {
            output: output.clone(),
            exit_code: exit_code.clone(),
            status: status.clone(),
            watch_tx,
        };
        let task = BackgroundTask {
            output,
            exit_code,
            status,
            description,
            child: Arc::new(Mutex::new(None)),
            status_watch: watch_rx,
        };
        self.insert(id, task);
        handle
    }
}

impl Default for BackgroundTaskStore {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
        }
    }
}

/// Handle for updating a proxy task from outside this crate.
pub struct ProxyHandle {
    output: Arc<Mutex<String>>,
    exit_code: Arc<Mutex<Option<i32>>>,
    status: Arc<Mutex<TaskStatus>>,
    watch_tx: watch::Sender<TaskStatus>,
}

impl ProxyHandle {
    /// Mark the proxy task as completed with its final output.
    pub fn complete(&self, output: String, success: bool) {
        let new_status = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        *self.output.lock().unwrap() = output;
        *self.status.lock().unwrap() = new_status.clone();
        *self.exit_code.lock().unwrap() = Some(if success { 0 } else { 1 });
        let _ = self.watch_tx.send(new_status);
    }
}
