use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tokio::process::Child;

pub mod spawn;
pub mod task_output;
pub mod task_stop;

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
