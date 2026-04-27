//! Partial-update payload for [`TaskStore::update`](crate::task_store::TaskStore::update).
//!
//! Each `Option<_>` field, when `Some`, replaces the corresponding field
//! on the target task. The two `Vec<_>` fields are unioned with the
//! existing values rather than replacing — they extend the dependency
//! graph rather than rewriting it.

use crate::types::{Task, TaskId, TaskStatus};

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
    pub(crate) fn apply(&self, task: &mut Task) {
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
