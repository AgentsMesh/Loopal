use std::time::{Duration, Instant};

use loopal_tool_invocation::{
    CancelCause, FailureKind, InvocationId, InvocationState, Outcome, ProgressSnapshot,
    StaleReason, ToolInvocation, TransitionCmd, transition,
};

fn fresh(now: Instant) -> ToolInvocation {
    let id = InvocationId::new("tc-1").unwrap();
    ToolInvocation::start(id, "Bash", "Bash(ls)", None, now)
}

#[test]
fn pending_to_running_via_progress() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let snap = ProgressSnapshot::new("line");
    let next = transition(
        inv,
        TransitionCmd::RecordProgress(snap),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert!(matches!(next.state, InvocationState::Running { .. }));
    assert_eq!(next.state.progress_tail(), Some("line"));
}

#[test]
fn running_to_running_updates_progress() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let p1 = ProgressSnapshot::new("first");
    let mid = transition(inv, TransitionCmd::RecordProgress(p1), t0).unwrap();
    let p2 = ProgressSnapshot::new("second");
    let next = transition(mid, TransitionCmd::RecordProgress(p2), t0).unwrap();
    assert_eq!(next.state.progress_tail(), Some("second"));
}

#[test]
fn pending_to_done_skips_running() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let outcome = Outcome::Success {
        content: "ok".into(),
    };
    let next = transition(
        inv,
        TransitionCmd::Complete(outcome),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert!(matches!(next.state, InvocationState::Done { .. }));
    assert!(!next.state.is_active());
}

#[test]
fn running_to_done() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let mid = transition(
        inv,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("")),
        t0,
    )
    .unwrap();
    let outcome = Outcome::Failure {
        error: "bad".into(),
        kind: FailureKind::ToolError,
    };
    let next = transition(
        mid,
        TransitionCmd::Complete(outcome),
        t0 + Duration::from_secs(2),
    )
    .unwrap();
    let Outcome::Failure { kind, .. } = next.state.outcome().unwrap() else {
        panic!("expected failure outcome")
    };
    assert_eq!(*kind, FailureKind::ToolError);
}

#[test]
fn pending_to_stale() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let next = transition(
        inv,
        TransitionCmd::MarkStale(StaleReason::TurnEnded),
        t0 + Duration::from_secs(3),
    )
    .unwrap();
    let InvocationState::Stale { reason, .. } = next.state else {
        panic!("expected stale")
    };
    assert_eq!(reason, StaleReason::TurnEnded);
}

#[test]
fn pending_to_cancelled() {
    let t0 = Instant::now();
    let next = transition(
        fresh(t0),
        TransitionCmd::Cancel(CancelCause::ParentCancelled),
        t0,
    )
    .unwrap();
    assert!(matches!(next.state, InvocationState::Cancelled { .. }));
}

#[test]
fn running_to_cancelled() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let mid = transition(
        inv,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("")),
        t0,
    )
    .unwrap();
    let next = transition(
        mid,
        TransitionCmd::Cancel(CancelCause::UserInterrupt),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert!(matches!(next.state, InvocationState::Cancelled { .. }));
}

#[test]
fn since_is_preserved_across_transitions() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let t1 = t0 + Duration::from_millis(500);
    let mid = transition(
        inv,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("")),
        t1,
    )
    .unwrap();
    assert_eq!(mid.started_at, t0);
    let t2 = t1 + Duration::from_secs(1);
    let done = transition(
        mid,
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t2,
    )
    .unwrap();
    assert_eq!(done.started_at, t0);
}

#[test]
fn id_and_name_unchanged_across_transitions() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let done = transition(
        inv,
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t0,
    )
    .unwrap();
    assert_eq!(done.id.as_str(), "tc-1");
    assert_eq!(done.name, "Bash");
}
