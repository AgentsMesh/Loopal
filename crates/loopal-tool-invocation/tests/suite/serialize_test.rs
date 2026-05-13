use std::time::Instant;

use loopal_tool_invocation::{
    FailureKind, InvocationId, InvocationState, Outcome, ProgressSnapshot, StaleReason,
    ToolInvocation, TransitionCmd, transition,
};

fn inv(state_name: &str) -> ToolInvocation {
    let t0 = Instant::now();
    let mut inv = ToolInvocation::start(
        InvocationId::new("tc-s").unwrap(),
        "Bash",
        "Bash(ls)",
        Some(serde_json::json!({"command": "ls"})),
        t0,
    );
    inv = match state_name {
        "Pending" => inv,
        "Running" => transition(
            inv,
            TransitionCmd::RecordProgress(ProgressSnapshot::new("hi")),
            t0,
        )
        .unwrap(),
        "DoneSuccess" => transition(
            inv,
            TransitionCmd::Complete(Outcome::Success {
                content: "ok".into(),
            }),
            t0,
        )
        .unwrap(),
        "DoneFailure" => transition(
            inv,
            TransitionCmd::Complete(Outcome::Failure {
                error: "boom".into(),
                kind: FailureKind::ToolError,
            }),
            t0,
        )
        .unwrap(),
        "Stale" => transition(
            inv,
            TransitionCmd::MarkStale(StaleReason::WatchdogTimeout),
            t0,
        )
        .unwrap(),
        _ => unreachable!(),
    };
    inv
}

#[test]
fn pending_roundtrip_preserves_id_and_name() {
    let original = inv("Pending");
    let json = serde_json::to_string(&original).unwrap();
    let back: ToolInvocation = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id.as_str(), "tc-s");
    assert_eq!(back.name, "Bash");
    assert!(matches!(back.state, InvocationState::Pending));
}

#[test]
fn done_success_roundtrip_preserves_outcome() {
    let original = inv("DoneSuccess");
    let json = serde_json::to_string(&original).unwrap();
    let back: ToolInvocation = serde_json::from_str(&json).unwrap();
    let Outcome::Success { content, .. } = back.state.outcome().unwrap() else {
        panic!("expected Success")
    };
    assert_eq!(content, "ok");
}

#[test]
fn done_failure_roundtrip_preserves_kind() {
    let original = inv("DoneFailure");
    let json = serde_json::to_string(&original).unwrap();
    let back: ToolInvocation = serde_json::from_str(&json).unwrap();
    let Outcome::Failure { error, kind } = back.state.outcome().unwrap() else {
        panic!("expected Failure")
    };
    assert_eq!(error, "boom");
    assert_eq!(*kind, FailureKind::ToolError);
}

#[test]
fn stale_roundtrip_preserves_reason() {
    let original = inv("Stale");
    let json = serde_json::to_string(&original).unwrap();
    let back: ToolInvocation = serde_json::from_str(&json).unwrap();
    let InvocationState::Stale { reason, .. } = back.state else {
        panic!("expected Stale")
    };
    assert_eq!(reason, StaleReason::WatchdogTimeout);
}

#[test]
fn state_tag_uses_snake_case() {
    let v = inv("Pending");
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["state"]["state"], "pending");
}

#[test]
fn outcome_tag_uses_snake_case() {
    let v = inv("DoneSuccess");
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["state"]["state"], "done");
    assert_eq!(json["state"]["outcome"]["type"], "success");
}

#[test]
fn instant_fields_skipped_in_wire() {
    let v = inv("DoneSuccess");
    let json = serde_json::to_value(&v).unwrap();
    assert!(json["state"].get("since").is_none());
    assert!(json["state"].get("until").is_none());
}

#[test]
fn stale_reason_display_is_human_readable() {
    assert_eq!(StaleReason::WatchdogTimeout.to_string(), "watchdog timeout");
    assert_eq!(StaleReason::TurnEnded.to_string(), "turn ended");
    assert_eq!(StaleReason::ConnectionLost.to_string(), "connection lost");
}

#[test]
fn cancel_cause_display_is_human_readable() {
    use loopal_tool_invocation::CancelCause;
    assert_eq!(CancelCause::UserInterrupt.to_string(), "user interrupt");
    assert_eq!(CancelCause::ParentCancelled.to_string(), "parent cancelled");
}

#[test]
fn failure_kind_display_is_human_readable() {
    assert_eq!(FailureKind::ToolError.to_string(), "tool error");
    assert_eq!(
        FailureKind::PermissionDenied.to_string(),
        "permission denied"
    );
    assert_eq!(FailureKind::Interrupted.to_string(), "interrupted");
    assert_eq!(FailureKind::Watchdog.to_string(), "watchdog");
}

#[test]
fn invocation_metadata_round_trips() {
    use loopal_tool_invocation::ToolResultMetadata;
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let inv = ToolInvocation {
        id: InvocationId::new("tc-meta").unwrap(),
        name: "Write".into(),
        summary: "Write(/tmp/x)".into(),
        input: None,
        started_at: now,
        state: InvocationState::Done {
            duration: Duration::from_millis(500),
            outcome: Outcome::Success {
                content: "ok".into(),
            },
        },
        batch_id: None,
        metadata: Some(ToolResultMetadata::bytes_written(1024)),
    };
    let json = serde_json::to_string(&inv).unwrap();
    let back: ToolInvocation = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.metadata.as_ref(),
        Some(&ToolResultMetadata::bytes_written(1024))
    );
    assert_eq!(back.state.duration(), Some(Duration::from_millis(500)));
}
