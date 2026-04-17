//! Tests for task_state: TasksChanged event handling.

use loopal_protocol::{AgentEvent, AgentEventPayload, TaskSnapshot, TaskSnapshotStatus};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

fn snapshot(id: &str, subject: &str, status: TaskSnapshotStatus) -> TaskSnapshot {
    TaskSnapshot {
        id: id.into(),
        subject: subject.into(),
        active_form: None,
        status,
        blocked_by: Vec::new(),
    }
}

fn emit_tasks_changed(state: &mut SessionState, tasks: Vec<TaskSnapshot>) {
    apply_event(
        state,
        AgentEvent::root(AgentEventPayload::TasksChanged { tasks }),
    );
}

#[test]
fn stores_task_snapshots() {
    let mut state = make_state();
    assert!(state.task_snapshots.is_empty());
    let tasks = vec![
        snapshot("1", "Task A", TaskSnapshotStatus::Pending),
        snapshot("2", "Task B", TaskSnapshotStatus::InProgress),
    ];
    emit_tasks_changed(&mut state, tasks);
    assert_eq!(state.task_snapshots.len(), 2);
    assert_eq!(state.task_snapshots[0].subject, "Task A");
    assert_eq!(
        state.task_snapshots[1].status,
        TaskSnapshotStatus::InProgress
    );
}

#[test]
fn replaces_previous_snapshots() {
    let mut state = make_state();
    emit_tasks_changed(
        &mut state,
        vec![snapshot("1", "Old", TaskSnapshotStatus::Pending)],
    );
    assert_eq!(state.task_snapshots.len(), 1);
    emit_tasks_changed(
        &mut state,
        vec![
            snapshot("1", "Old", TaskSnapshotStatus::Completed),
            snapshot("2", "New", TaskSnapshotStatus::Pending),
        ],
    );
    assert_eq!(state.task_snapshots.len(), 2);
    assert_eq!(
        state.task_snapshots[0].status,
        TaskSnapshotStatus::Completed
    );
}

#[test]
fn empty_tasks_clears_snapshots() {
    let mut state = make_state();
    emit_tasks_changed(
        &mut state,
        vec![snapshot("1", "X", TaskSnapshotStatus::InProgress)],
    );
    assert_eq!(state.task_snapshots.len(), 1);
    emit_tasks_changed(&mut state, Vec::new());
    assert!(state.task_snapshots.is_empty());
}

#[test]
fn session_resumed_clears_task_snapshots() {
    let mut state = make_state();
    emit_tasks_changed(
        &mut state,
        vec![snapshot(
            "stale",
            "pre-resume",
            TaskSnapshotStatus::InProgress,
        )],
    );
    assert_eq!(state.task_snapshots.len(), 1);

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SessionResumed {
            session_id: "new-sid".into(),
            message_count: 0,
        }),
    );

    assert!(
        state.task_snapshots.is_empty(),
        "SessionResumed must clear stale task cache"
    );
}
