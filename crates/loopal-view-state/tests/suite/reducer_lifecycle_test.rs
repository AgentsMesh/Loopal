//! Constructor + state-accessor tests for `ViewStateReducer`.
//!
//! Pins down: rev starts at 0/1 depending on entry path, snapshot
//! mirrors current rev, state accessor reflects mutations.

use loopal_protocol::{AgentEventPayload, AgentStateSnapshot, TaskSnapshot, TaskSnapshotStatus};
use loopal_view_state::ViewStateReducer;

#[test]
fn new_starts_with_rev_zero() {
    let r = ViewStateReducer::new("root");
    assert_eq!(r.rev(), 0);
    assert_eq!(r.state().agent.name, "root");
    assert!(r.state().tasks.is_empty());
}

#[test]
fn from_snapshot_starts_with_rev_one() {
    let snap = AgentStateSnapshot {
        tasks: vec![TaskSnapshot {
            id: "t1".into(),
            subject: "preserved".into(),
            active_form: None,
            status: TaskSnapshotStatus::Pending,
            blocked_by: vec![],
        }],
        crons: vec![],
        bg_tasks: vec![],
    };
    let r = ViewStateReducer::from_snapshot("root", snap);
    assert_eq!(r.rev(), 1);
    assert_eq!(r.state().tasks.len(), 1);
    assert_eq!(r.state().tasks[0].subject, "preserved");
}

#[test]
fn snapshot_method_mirrors_current_rev_and_state() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);

    let snap = r.snapshot();
    assert_eq!(snap.rev, 1);
    assert_eq!(
        snap.state.agent.observable.status,
        loopal_protocol::AgentStatus::Running
    );
}

#[test]
fn rev_is_strictly_monotonic_across_observable_events() {
    let mut r = ViewStateReducer::new("root");
    let mut last_rev = 0;
    for evt in [
        AgentEventPayload::Started,
        AgentEventPayload::Running,
        AgentEventPayload::AwaitingInput,
        AgentEventPayload::Finished,
    ] {
        r.apply(evt);
        assert!(r.rev() > last_rev);
        last_rev = r.rev();
    }
    assert_eq!(r.rev(), 4);
}

#[test]
fn rev_unchanged_when_event_is_non_observable() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    let after_running = r.rev();

    let result = r.apply(AgentEventPayload::TurnDiffSummary {
        modified_files: vec![],
    });
    assert!(result.is_none());
    assert_eq!(r.rev(), after_running);
}
