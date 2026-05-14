use std::time::Instant;

use loopal_tool_invocation::{
    CancelCause, InvocationId, Outcome, ProgressSnapshot, StaleReason, ToolInvocation,
    TransitionCmd, TransitionError, transition,
};

fn fresh(now: Instant) -> ToolInvocation {
    let id = InvocationId::new("tc-1").unwrap();
    ToolInvocation::start(id, "Bash", "Bash(ls)", None, now)
}

fn done_state(t0: Instant) -> ToolInvocation {
    transition(
        fresh(t0),
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t0,
    )
    .unwrap()
}

fn stale_state(t0: Instant) -> ToolInvocation {
    transition(
        fresh(t0),
        TransitionCmd::MarkStale(StaleReason::TurnEnded),
        t0,
    )
    .unwrap()
}

fn cancelled_state(t0: Instant) -> ToolInvocation {
    transition(
        fresh(t0),
        TransitionCmd::Cancel(CancelCause::UserInterrupt),
        t0,
    )
    .unwrap()
}

fn assert_invalid(result: Result<ToolInvocation, TransitionError>, expected_state: &'static str) {
    match result {
        Err(TransitionError::Invalid { state, .. }) => assert_eq!(state, expected_state),
        other => panic!("expected Invalid({expected_state}), got {other:?}"),
    }
}

#[test]
fn done_rejects_record_progress() {
    let t0 = Instant::now();
    let err = transition(
        done_state(t0),
        TransitionCmd::RecordProgress(ProgressSnapshot::new("late")),
        t0,
    );
    assert_invalid(err, "Done");
}

#[test]
fn done_rejects_complete() {
    let t0 = Instant::now();
    let err = transition(
        done_state(t0),
        TransitionCmd::Complete(Outcome::Success {
            content: "again".into(),
        }),
        t0,
    );
    assert_invalid(err, "Done");
}

#[test]
fn done_rejects_mark_stale() {
    let t0 = Instant::now();
    let err = transition(
        done_state(t0),
        TransitionCmd::MarkStale(StaleReason::TurnEnded),
        t0,
    );
    assert_invalid(err, "Done");
}

#[test]
fn done_rejects_cancel() {
    let t0 = Instant::now();
    let err = transition(
        done_state(t0),
        TransitionCmd::Cancel(CancelCause::UserInterrupt),
        t0,
    );
    assert_invalid(err, "Done");
}

#[test]
fn stale_rejects_record_progress() {
    let t0 = Instant::now();
    let err = transition(
        stale_state(t0),
        TransitionCmd::RecordProgress(ProgressSnapshot::new("late")),
        t0,
    );
    assert_invalid(err, "Stale");
}

#[test]
fn stale_rejects_complete() {
    let t0 = Instant::now();
    let err = transition(
        stale_state(t0),
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t0,
    );
    assert_invalid(err, "Stale");
}

#[test]
fn stale_rejects_mark_stale() {
    let t0 = Instant::now();
    let err = transition(
        stale_state(t0),
        TransitionCmd::MarkStale(StaleReason::WatchdogTimeout),
        t0,
    );
    assert_invalid(err, "Stale");
}

#[test]
fn stale_rejects_cancel() {
    let t0 = Instant::now();
    let err = transition(
        stale_state(t0),
        TransitionCmd::Cancel(CancelCause::UserInterrupt),
        t0,
    );
    assert_invalid(err, "Stale");
}

#[test]
fn cancelled_rejects_record_progress() {
    let t0 = Instant::now();
    let err = transition(
        cancelled_state(t0),
        TransitionCmd::RecordProgress(ProgressSnapshot::new("late")),
        t0,
    );
    assert_invalid(err, "Cancelled");
}

#[test]
fn cancelled_rejects_complete() {
    let t0 = Instant::now();
    let err = transition(
        cancelled_state(t0),
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t0,
    );
    assert_invalid(err, "Cancelled");
}

#[test]
fn cancelled_rejects_mark_stale() {
    let t0 = Instant::now();
    let err = transition(
        cancelled_state(t0),
        TransitionCmd::MarkStale(StaleReason::TurnEnded),
        t0,
    );
    assert_invalid(err, "Cancelled");
}

#[test]
fn cancelled_rejects_re_cancel() {
    let t0 = Instant::now();
    let err = transition(
        cancelled_state(t0),
        TransitionCmd::Cancel(CancelCause::ParentCancelled),
        t0,
    );
    assert_invalid(err, "Cancelled");
}
