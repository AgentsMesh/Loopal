use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_agent::types::{Task, TaskStatus};
use loopal_agent::{SessionScopedTaskStorage, TaskPatch, TaskStore};
use serde_json::json;

struct FailingSaveStorage {
    attempts: AtomicUsize,
}

#[async_trait]
impl SessionScopedTaskStorage for FailingSaveStorage {
    async fn load(&self, _: &str) -> std::io::Result<(Vec<Task>, u64)> {
        Ok((Vec::new(), 1))
    }

    async fn save_all(&self, _: &str, _: &[Task]) -> std::io::Result<()> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(std::io::Error::other("expected save failure"))
    }
}

#[tokio::test]
async fn full_patch_and_duplicate_relations_survive_save_failures() {
    let storage = Arc::new(FailingSaveStorage {
        attempts: AtomicUsize::new(0),
    });
    let store = TaskStore::with_session_storage(storage.clone());
    store.switch_session("branch-session").await.unwrap();
    let task = store.create("old subject", "old description").await;
    let related = store.create("related", "").await;

    let updated = store
        .update(
            &task.id,
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                subject: Some("new subject".into()),
                description: Some("new description".into()),
                active_form: Some("working".into()),
                owner: Some(None),
                add_blocked_by: vec![related.id.clone()],
                add_blocks: vec![related.id.clone()],
                metadata: Some(json!({"source": "branch-test"})),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.subject, "new subject");
    assert_eq!(updated.description, "new description");
    assert_eq!(updated.metadata, json!({"source": "branch-test"}));

    let deduped = store
        .update(
            &task.id,
            TaskPatch {
                add_blocked_by: vec![related.id.clone()],
                add_blocks: vec![related.id.clone()],
                ..TaskPatch::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(deduped.blocked_by, vec![related.id.clone()]);
    assert_eq!(deduped.blocks, vec![related.id]);
    assert_eq!(storage.attempts.load(Ordering::SeqCst), 4);
}
