//! File-backed `SessionScopedTaskStorage` — `<root>/<id>/tasks/tasks.json`.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::task_session_storage::{SessionScopedTaskStorage, TaskLoad};
use crate::types::Task;

pub struct FileScopedTaskStore {
    sessions_root: PathBuf,
}

impl FileScopedTaskStore {
    pub fn new(sessions_root: PathBuf) -> Self {
        Self { sessions_root }
    }

    pub fn root(&self) -> &Path {
        &self.sessions_root
    }

    fn dir_for(&self, session_id: &str) -> PathBuf {
        self.sessions_root.join(session_id).join("tasks")
    }

    fn file_for(&self, session_id: &str) -> PathBuf {
        self.dir_for(session_id).join("tasks.json")
    }
}

#[async_trait]
impl SessionScopedTaskStorage for FileScopedTaskStore {
    async fn load(&self, session_id: &str) -> std::io::Result<TaskLoad> {
        let path = self.file_for(session_id);
        tokio::task::spawn_blocking(move || load_blocking(&path))
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?
    }

    async fn save_all(&self, session_id: &str, tasks: &[Task]) -> std::io::Result<()> {
        let dir = self.dir_for(session_id);
        let file = self.file_for(session_id);
        let payload = tasks.to_vec();
        tokio::task::spawn_blocking(move || save_blocking(&dir, &file, &payload))
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?
    }
}

fn load_blocking(path: &Path) -> std::io::Result<TaskLoad> {
    if !path.exists() {
        return Ok((Vec::new(), 1));
    }
    let content = std::fs::read_to_string(path)?;
    let tasks: Vec<Task> = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let max_id = tasks
        .iter()
        .filter_map(|t| t.id.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    Ok((tasks, max_id + 1))
}

fn save_blocking(dir: &Path, file: &Path, tasks: &[Task]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(tasks)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(file, json)
}
