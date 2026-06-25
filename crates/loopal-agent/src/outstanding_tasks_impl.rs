use async_trait::async_trait;
use loopal_tool_api::OutstandingTasksDigest;

use crate::shared::AgentShared;
use crate::types::{Task, TaskStatus};

#[async_trait]
impl OutstandingTasksDigest for AgentShared {
    async fn outstanding_tasks_digest(&self) -> Option<String> {
        outstanding_digest(&self.task_store.list().await)
    }
}

fn outstanding_digest(tasks: &[Task]) -> Option<String> {
    let mut lines = Vec::new();
    for t in tasks {
        let status = match t.status {
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Pending => "pending",
            TaskStatus::Completed | TaskStatus::Deleted => continue,
        };
        let af = t
            .active_form
            .as_deref()
            .map(|a| format!(" — {a}"))
            .unwrap_or_default();
        lines.push(format!("- #{} [{}] {}{}", t.id, status, t.subject, af));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "\n\n## Outstanding tasks (carry-forward — reconcile before starting new work)\n{}",
        lines.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::outstanding_digest;
    use crate::types::{Task, TaskStatus};

    fn task(id: &str, status: TaskStatus, subject: &str, af: Option<&str>) -> Task {
        Task {
            id: id.into(),
            subject: subject.into(),
            description: String::new(),
            active_form: af.map(String::from),
            status,
            owner: None,
            blocked_by: vec![],
            blocks: vec![],
            metadata: serde_json::Value::Null,
            created_at: String::new(),
        }
    }

    #[test]
    fn lists_only_non_completed_with_status_and_active_form() {
        let tasks = vec![
            task("1", TaskStatus::Completed, "done", None),
            task("10", TaskStatus::InProgress, "produce", Some("Producing")),
            task("60", TaskStatus::Pending, "outreach", None),
            task("99", TaskStatus::Deleted, "gone", None),
        ];
        let d = outstanding_digest(&tasks).expect("non-empty");
        assert!(d.contains("Outstanding tasks"));
        assert!(d.contains("- #10 [in_progress] produce — Producing"));
        assert!(d.contains("- #60 [pending] outreach"));
        assert!(!d.contains("done"), "completed excluded");
        assert!(!d.contains("gone"), "deleted excluded");
    }

    #[test]
    fn none_when_no_outstanding() {
        assert!(outstanding_digest(&[task("1", TaskStatus::Completed, "x", None)]).is_none());
        assert!(outstanding_digest(&[]).is_none());
    }
}
