use async_trait::async_trait;

/// Surfaces the not-yet-completed task list as a compact markdown digest.
///
/// The runtime appends this to the compaction summary so the agent does not
/// lose sight of its `in_progress` / `pending` work when the conversational
/// record of `TaskCreate` / `TaskUpdate` is summarized away.
#[async_trait]
pub trait OutstandingTasksDigest: Send + Sync {
    /// `None` when there are no outstanding tasks.
    async fn outstanding_tasks_digest(&self) -> Option<String>;
}
