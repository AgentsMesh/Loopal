use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tokio::process::Child;
use tokio::sync::watch;

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

static STORE: LazyLock<Mutex<HashMap<String, BackgroundTask>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn store() -> &'static Mutex<HashMap<String, BackgroundTask>> {
    &STORE
}

pub fn generate_task_id() -> String {
    format!("bg_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Register a proxy task for a background agent process.
/// Returns a handle for updating status when the agent completes.
pub fn register_proxy(id: String, description: String) -> ProxyHandle {
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
    store().lock().unwrap().insert(id, task);
    handle
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
        // Notify watchers immediately — no polling needed.
        let _ = self.watch_tx.send(new_status);
    }
}
