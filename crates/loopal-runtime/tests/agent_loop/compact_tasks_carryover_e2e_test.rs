use std::sync::Arc;

use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_tool_api::OutstandingTasksDigest;
use loopal_turn::TurnStep;

struct FixedTasks;

#[async_trait::async_trait]
impl OutstandingTasksDigest for FixedTasks {
    async fn outstanding_tasks_digest(&self) -> Option<String> {
        Some("\n\n## Outstanding tasks\n- #42 [in_progress] sentinel task".into())
    }
}

#[tokio::test]
async fn compaction_summary_carries_outstanding_tasks_forward() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("<summary>compacted body</summary>")])
        .outstanding_tasks(Arc::new(FixedTasks))
        .messages(
            (1..=5)
                .map(|i| Message::user(&format!("turn {i}")))
                .collect(),
        )
        .build()
        .await;

    let result = h.runner.force_compact(None).await;
    assert!(
        matches!(result, Ok(true)),
        "compaction should succeed: {result:?}"
    );

    let summary = h
        .runner
        .turns
        .store()
        .turns()
        .iter()
        .flat_map(|t| &t.body.steps)
        .find_map(|s| match s {
            TurnStep::CompactionSummary(cs) => Some(cs.summary_text.clone()),
            _ => None,
        })
        .expect("a CompactionSummary step");
    assert!(
        summary.contains("#42 [in_progress] sentinel task"),
        "compaction summary must carry the outstanding task list forward; got: {summary}"
    );
}
