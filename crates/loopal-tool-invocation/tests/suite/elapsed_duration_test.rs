use std::time::{Duration, Instant};

use loopal_tool_invocation::{
    InvocationId, Outcome, ProgressSnapshot, StaleReason, ToolInvocation, TransitionCmd, transition,
};

fn fresh(now: Instant) -> ToolInvocation {
    ToolInvocation::start(
        InvocationId::new("tc-d").unwrap(),
        "Bash",
        "Bash(x)",
        None,
        now,
    )
}

#[test]
fn pending_elapsed_grows_with_now() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let later = t0 + Duration::from_secs(3);
    assert_eq!(inv.elapsed(later), Duration::from_secs(3));
}

#[test]
fn pending_has_no_duration() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    assert_eq!(inv.state.duration(), None);
}

#[test]
fn running_has_no_duration_but_has_elapsed() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    let inv = transition(
        inv,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("")),
        t0,
    )
    .unwrap();
    let later = t0 + Duration::from_secs(2);
    assert_eq!(inv.state.duration(), None);
    assert_eq!(inv.elapsed(later), Duration::from_secs(2));
}

#[test]
fn done_freezes_elapsed_at_until() {
    let t0 = Instant::now();
    let until = t0 + Duration::from_secs(5);
    let inv = fresh(t0);
    let inv = transition(
        inv,
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        until,
    )
    .unwrap();
    let now = until + Duration::from_secs(100);
    assert_eq!(inv.elapsed(now), Duration::from_secs(5));
    assert_eq!(inv.state.duration(), Some(Duration::from_secs(5)));
}

#[test]
fn stale_freezes_elapsed_at_marked() {
    let t0 = Instant::now();
    let marked = t0 + Duration::from_secs(7);
    let inv = fresh(t0);
    let inv = transition(
        inv,
        TransitionCmd::MarkStale(StaleReason::WatchdogTimeout),
        marked,
    )
    .unwrap();
    let now = marked + Duration::from_secs(50);
    assert_eq!(inv.elapsed(now), Duration::from_secs(7));
    assert_eq!(inv.state.duration(), Some(Duration::from_secs(7)));
}

#[test]
fn is_active_matches_state() {
    let t0 = Instant::now();
    let pending = fresh(t0);
    assert!(pending.state.is_active());
    assert!(!pending.state.is_terminal());

    let running = transition(
        pending,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("")),
        t0,
    )
    .unwrap();
    assert!(running.state.is_active());

    let done = transition(
        running,
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t0,
    )
    .unwrap();
    assert!(!done.state.is_active());
    assert!(done.state.is_terminal());
}

#[test]
fn variant_name_stable() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    assert_eq!(inv.state.variant_name(), "Pending");
    let inv = transition(
        inv,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("")),
        t0,
    )
    .unwrap();
    assert_eq!(inv.state.variant_name(), "Running");
}

#[test]
fn clock_skew_does_not_panic() {
    let t0 = Instant::now() + Duration::from_secs(100);
    let inv = fresh(t0);
    let earlier = Instant::now();
    assert_eq!(inv.elapsed(earlier), Duration::ZERO);
}

#[test]
fn progress_tail_only_visible_in_running() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    assert_eq!(inv.state.progress_tail(), None);
    let inv = transition(
        inv,
        TransitionCmd::RecordProgress(ProgressSnapshot::new("tail!")),
        t0,
    )
    .unwrap();
    assert_eq!(inv.state.progress_tail(), Some("tail!"));
    let inv = transition(
        inv,
        TransitionCmd::Complete(Outcome::Success { content: "".into() }),
        t0,
    )
    .unwrap();
    assert_eq!(inv.state.progress_tail(), None);
}

#[test]
fn done_exposes_outcome() {
    let t0 = Instant::now();
    let inv = fresh(t0);
    assert!(inv.state.outcome().is_none());
    let inv = transition(
        inv,
        TransitionCmd::Complete(Outcome::Success {
            content: "ok".into(),
        }),
        t0,
    )
    .unwrap();
    assert_eq!(inv.state.outcome().unwrap().content(), "ok");
}
