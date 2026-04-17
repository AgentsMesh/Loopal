//! Tests for bg_task_state: incremental event handling for background tasks.

use loopal_protocol::{AgentEvent, AgentEventPayload, BgTaskStatus};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

fn emit_spawned(state: &mut SessionState, id: &str, desc: &str) {
    apply_event(
        state,
        AgentEvent::root(AgentEventPayload::BgTaskSpawned {
            id: id.into(),
            description: desc.into(),
        }),
    );
}

fn emit_output(state: &mut SessionState, id: &str, delta: &str) {
    apply_event(
        state,
        AgentEvent::root(AgentEventPayload::BgTaskOutput {
            id: id.into(),
            output_delta: delta.into(),
        }),
    );
}

fn emit_completed(state: &mut SessionState, id: &str, status: BgTaskStatus, output: &str) {
    let code = if status == BgTaskStatus::Completed {
        0
    } else {
        1
    };
    apply_event(
        state,
        AgentEvent::root(AgentEventPayload::BgTaskCompleted {
            id: id.into(),
            status,
            exit_code: Some(code),
            output: output.into(),
        }),
    );
}

#[test]
fn spawned_creates_running_entry() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_1", "npm install");
    let task = &state.bg_tasks["bg_1"];
    assert_eq!(task.status, BgTaskStatus::Running);
    assert_eq!(task.description, "npm install");
    assert!(task.output.is_empty());
}

#[test]
fn output_appends_delta_when_running() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_1", "build");
    emit_output(&mut state, "bg_1", "line 1\n");
    emit_output(&mut state, "bg_1", "line 2\n");
    assert_eq!(state.bg_tasks["bg_1"].output, "line 1\nline 2\n");
}

#[test]
fn output_ignored_after_completion() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_1", "build");
    emit_output(&mut state, "bg_1", "partial\n");
    emit_completed(&mut state, "bg_1", BgTaskStatus::Completed, "full output");
    emit_output(&mut state, "bg_1", "late delta\n");
    assert_eq!(state.bg_tasks["bg_1"].output, "full output");
}

#[test]
fn completed_overwrites_accumulated_output() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_1", "build");
    emit_output(&mut state, "bg_1", "delta1\n");
    emit_completed(&mut state, "bg_1", BgTaskStatus::Failed, "authoritative");
    let task = &state.bg_tasks["bg_1"];
    assert_eq!(task.status, BgTaskStatus::Failed);
    assert_eq!(task.exit_code, Some(1));
    assert_eq!(task.output, "authoritative");
}

#[test]
fn completed_before_spawned_creates_entry() {
    let mut state = make_state();
    emit_completed(&mut state, "bg_1", BgTaskStatus::Completed, "done");
    assert!(state.bg_tasks.contains_key("bg_1"));
    assert_eq!(state.bg_tasks["bg_1"].status, BgTaskStatus::Completed);
    assert_eq!(state.bg_tasks["bg_1"].output, "done");
}

#[test]
fn duplicate_spawned_is_idempotent() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_1", "first");
    emit_output(&mut state, "bg_1", "data\n");
    emit_spawned(&mut state, "bg_1", "second");
    assert_eq!(state.bg_tasks["bg_1"].description, "first");
    assert_eq!(state.bg_tasks["bg_1"].output, "data\n");
}

#[test]
fn output_for_unknown_task_is_noop() {
    let mut state = make_state();
    emit_output(&mut state, "bg_999", "orphan data");
    assert!(!state.bg_tasks.contains_key("bg_999"));
}

#[test]
fn multiple_tasks_independent() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_1", "task1");
    emit_spawned(&mut state, "bg_2", "task2");
    emit_output(&mut state, "bg_1", "out1\n");
    emit_output(&mut state, "bg_2", "out2\n");
    assert_eq!(state.bg_tasks["bg_1"].output, "out1\n");
    assert_eq!(state.bg_tasks["bg_2"].output, "out2\n");
}

#[test]
fn insertion_order_preserved() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_3", "third");
    emit_spawned(&mut state, "bg_1", "first");
    emit_spawned(&mut state, "bg_2", "second");
    let ids: Vec<&str> = state.bg_tasks.keys().map(|s| s.as_str()).collect();
    assert_eq!(ids, vec!["bg_3", "bg_1", "bg_2"]);
}

#[test]
fn session_resumed_clears_bg_tasks() {
    let mut state = make_state();
    emit_spawned(&mut state, "bg_stale", "from old session");
    assert_eq!(state.bg_tasks.len(), 1);

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SessionResumed {
            session_id: "new-sid".into(),
            message_count: 0,
        }),
    );

    assert!(
        state.bg_tasks.is_empty(),
        "SessionResumed must clear stale bg_tasks"
    );
}
