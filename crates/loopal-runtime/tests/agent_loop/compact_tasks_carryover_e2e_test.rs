use std::sync::Arc;

use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_tool_api::OutstandingTasksDigest;
use loopal_turn::TurnStep;

struct FixedTasks;

struct BlockingTasks {
    entered: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

#[async_trait::async_trait]
impl OutstandingTasksDigest for FixedTasks {
    async fn outstanding_tasks_digest(&self) -> Option<String> {
        Some("\n\n## Outstanding tasks\n- #42 [in_progress] sentinel task".into())
    }
}

#[async_trait::async_trait]
impl OutstandingTasksDigest for BlockingTasks {
    async fn outstanding_tasks_digest(&self) -> Option<String> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            let _ = entered.send(());
        }
        std::future::pending().await
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

#[tokio::test]
async fn cancel_after_summary_before_persist_aborts_at_commit_boundary() {
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let blocking_tasks = Arc::new(BlockingTasks {
        entered: std::sync::Mutex::new(Some(entered_tx)),
    });
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn(
            "<summary>must not commit</summary>",
        )])
        .outstanding_tasks(blocking_tasks)
        .messages(
            (1..=5)
                .map(|i| Message::user(&format!("turn {i}")))
                .collect(),
        )
        .build()
        .await;
    let session_ctrl = h.session_ctrl.clone();
    let mut runner = h.runner;
    let turns_before = runner.turns.store().turns().len();
    let compact_task = tokio::spawn(async move {
        let result = runner.force_compact(None).await;
        (runner, result)
    });

    tokio::time::timeout(std::time::Duration::from_secs(1), entered_rx)
        .await
        .expect("compaction must reach the pre-commit task digest")
        .expect("task digest entry sender");
    session_ctrl.interrupt();

    let (runner, result) = tokio::time::timeout(std::time::Duration::from_secs(1), compact_task)
        .await
        .expect("cancellation must abort the blocked pre-commit digest")
        .expect("compaction task must not panic");
    assert!(matches!(result, Ok(false)), "cancel result: {result:?}");
    assert_eq!(runner.turns.store().turns().len(), turns_before);
    assert!(runner.turns.store().turns().iter().all(|turn| {
        turn.body
            .steps
            .iter()
            .all(|step| !matches!(step, TurnStep::CompactionSummary(_)))
    }));
}
