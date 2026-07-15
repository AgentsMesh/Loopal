use std::sync::Arc;
use std::time::Duration;

use loopal_agent::{InMemoryTaskStorage, TaskStore};
use loopal_protocol::AgentEventPayload;

use super::cron_bridge_helpers::CaptureFrontend;

#[tokio::test]
async fn emits_loaded_tasks_on_initial_spawn() {
    let storage = Arc::new(InMemoryTaskStorage::new());
    let writer = TaskStore::with_session_storage(storage.clone());
    writer.switch_session("persisted").await.unwrap();
    let task = writer.create("Restored task", "survives restart").await;
    let store = Arc::new(TaskStore::with_session_storage(storage));
    store.switch_session("persisted").await.unwrap();
    let receiver = store.subscribe();
    let (frontend, events) = CaptureFrontend::new();

    let bridge =
        loopal_agent_server::testing::task_bridge_spawn(receiver, store, Arc::new(frontend));
    tokio::time::sleep(Duration::from_millis(80)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let tasks = captured
        .iter()
        .find_map(|event| match event {
            AgentEventPayload::TasksChanged { tasks } => Some(tasks),
            _ => None,
        })
        .expect("initial TasksChanged");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, task.id);
    assert_eq!(tasks[0].subject, "Restored task");
}
