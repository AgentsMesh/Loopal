use std::path::PathBuf;
use std::sync::Mutex;

use crate::types::{Task, TaskId, TaskStatus};

/// File-backed task store with in-memory cache.
///
/// Tasks are persisted as individual JSON files under `base_dir/`.
/// All mutations are serialized through a `Mutex` for thread safety.
pub struct TaskStore {
    base_dir: PathBuf,
    inner: Mutex<TaskStoreInner>,
}

struct TaskStoreInner {
    tasks: Vec<Task>,
    next_id: u64,
}

impl TaskStore {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        let (tasks, next_id) = Self::load_from_disk(&base_dir);
        Self {
            base_dir,
            inner: Mutex::new(TaskStoreInner { tasks, next_id }),
        }
    }

    /// Create a new task. Returns the created task.
    pub fn create(&self, subject: &str, description: &str) -> Task {
        let mut inner = self.inner.lock().expect("lock poisoned");
        let id = inner.next_id.to_string();
        inner.next_id += 1;

        let task = Task {
            id: id.clone(),
            subject: subject.to_string(),
            description: description.to_string(),
            active_form: None,
            status: TaskStatus::Pending,
            owner: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            metadata: serde_json::Value::Object(Default::default()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        inner.tasks.push(task.clone());
        self.persist(&inner);
        task
    }

    /// Get a task by ID.
    pub fn get(&self, id: &str) -> Option<Task> {
        let inner = self.inner.lock().expect("lock poisoned");
        inner.tasks.iter().find(|t| t.id == id).cloned()
    }

    /// List all non-deleted tasks.
    pub fn list(&self) -> Vec<Task> {
        let inner = self.inner.lock().expect("lock poisoned");
        inner
            .tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Deleted)
            .cloned()
            .collect()
    }

    /// Update a task. Returns the updated task or `None` if not found.
    pub fn update(&self, id: &str, patch: TaskPatch) -> Option<Task> {
        let mut inner = self.inner.lock().expect("lock poisoned");
        let task = inner.tasks.iter_mut().find(|t| t.id == id)?;
        patch.apply(task);
        let updated = task.clone();
        self.persist(&inner);
        Some(updated)
    }

    fn persist(&self, inner: &TaskStoreInner) {
        let path = self.base_dir.join("tasks.json");
        let json = serde_json::to_string_pretty(&inner.tasks).unwrap_or_default();
        std::fs::write(path, json).ok();
    }

    fn load_from_disk(dir: &std::path::Path) -> (Vec<Task>, u64) {
        let path = dir.join("tasks.json");
        if !path.exists() {
            return (Vec::new(), 1);
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let tasks: Vec<Task> = serde_json::from_str(&content).unwrap_or_default();
        let max_id = tasks
            .iter()
            .filter_map(|t| t.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        (tasks, max_id + 1)
    }
}

/// Partial update fields for a task.
#[derive(Default)]
pub struct TaskPatch {
    pub status: Option<TaskStatus>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub active_form: Option<String>,
    pub owner: Option<Option<String>>,
    pub add_blocked_by: Vec<TaskId>,
    pub add_blocks: Vec<TaskId>,
    pub metadata: Option<serde_json::Value>,
}

impl TaskPatch {
    fn apply(&self, task: &mut Task) {
        if let Some(ref s) = self.status {
            task.status = s.clone();
        }
        if let Some(ref s) = self.subject {
            task.subject = s.clone();
        }
        if let Some(ref d) = self.description {
            task.description = d.clone();
        }
        if let Some(ref af) = self.active_form {
            task.active_form = Some(af.clone());
        }
        if let Some(ref o) = self.owner {
            task.owner = o.clone();
        }
        for id in &self.add_blocked_by {
            if !task.blocked_by.contains(id) {
                task.blocked_by.push(id.clone());
            }
        }
        for id in &self.add_blocks {
            if !task.blocks.contains(id) {
                task.blocks.push(id.clone());
            }
        }
        if let Some(ref m) = self.metadata {
            task.metadata = m.clone();
        }
    }
}
