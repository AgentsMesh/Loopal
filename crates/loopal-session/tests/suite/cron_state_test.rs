//! Tests for cron_state: CronsChanged event handling.

use loopal_protocol::{AgentEvent, AgentEventPayload, CronJobSnapshot};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

fn snapshot(id: &str, prompt: &str, next_ms: Option<i64>) -> CronJobSnapshot {
    CronJobSnapshot {
        id: id.into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: prompt.into(),
        recurring: true,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: next_ms,
    }
}

fn emit_crons_changed(state: &mut SessionState, crons: Vec<CronJobSnapshot>) {
    apply_event(
        state,
        AgentEvent::root(AgentEventPayload::CronsChanged { crons }),
    );
}

#[test]
fn stores_cron_snapshots() {
    let mut state = make_state();
    assert!(state.cron_snapshots.is_empty());
    let crons = vec![
        snapshot("abc12345", "cleanup cache", Some(1_700_000_000_000)),
        snapshot("def67890", "daily report", None),
    ];
    emit_crons_changed(&mut state, crons);
    assert_eq!(state.cron_snapshots.len(), 2);
    assert_eq!(state.cron_snapshots[0].id, "abc12345");
    assert_eq!(state.cron_snapshots[1].next_fire_unix_ms, None);
}

#[test]
fn replaces_previous_snapshots() {
    let mut state = make_state();
    emit_crons_changed(
        &mut state,
        vec![snapshot("old1", "old", Some(1_700_000_000_000))],
    );
    assert_eq!(state.cron_snapshots.len(), 1);
    emit_crons_changed(
        &mut state,
        vec![
            snapshot("new1", "new-one", Some(1_700_000_001_000)),
            snapshot("new2", "new-two", Some(1_700_000_002_000)),
        ],
    );
    assert_eq!(state.cron_snapshots.len(), 2);
    assert_eq!(state.cron_snapshots[0].id, "new1");
}

#[test]
fn empty_crons_clears_snapshots() {
    let mut state = make_state();
    emit_crons_changed(&mut state, vec![snapshot("x", "x", None)]);
    assert_eq!(state.cron_snapshots.len(), 1);
    emit_crons_changed(&mut state, Vec::new());
    assert!(state.cron_snapshots.is_empty());
}

#[test]
fn unrelated_event_preserves_snapshots() {
    let mut state = make_state();
    emit_crons_changed(&mut state, vec![snapshot("keep", "kept", None)]);
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Stream {
            text: "ignored".into(),
        }),
    );
    assert_eq!(state.cron_snapshots.len(), 1);
    assert_eq!(state.cron_snapshots[0].id, "keep");
}

#[test]
fn multiple_emit_last_wins() {
    let mut state = make_state();
    emit_crons_changed(&mut state, vec![snapshot("a", "a", None)]);
    emit_crons_changed(&mut state, vec![snapshot("b", "b", None)]);
    emit_crons_changed(&mut state, vec![snapshot("c", "c", None)]);
    assert_eq!(state.cron_snapshots.len(), 1);
    assert_eq!(state.cron_snapshots[0].id, "c");
}

#[test]
fn has_scheduled_crons_helper() {
    assert!(!loopal_session::cron_state::has_scheduled_crons(&[]));
    let one = vec![snapshot("a", "a", None)];
    assert!(loopal_session::cron_state::has_scheduled_crons(&one));
}

#[test]
fn session_resumed_clears_cron_snapshots() {
    let mut state = make_state();
    emit_crons_changed(
        &mut state,
        vec![snapshot("pre-resume", "stale data", Some(100))],
    );
    assert_eq!(state.cron_snapshots.len(), 1);

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SessionResumed {
            session_id: "new-sid".into(),
            message_count: 0,
        }),
    );

    assert!(
        state.cron_snapshots.is_empty(),
        "SessionResumed must clear stale cron cache"
    );
}
