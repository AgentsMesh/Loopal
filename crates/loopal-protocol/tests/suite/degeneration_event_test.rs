use chrono::{TimeZone, Utc};
use loopal_protocol::event_summary::{
    ContinuationGateSummary, DegenerationSignal, DegenerationSummary, GateCloseReason,
};
use loopal_protocol::{AgentEvent, AgentEventPayload};

#[test]
fn degeneration_signal_serde_uses_snake_case() {
    let pairs = [
        (DegenerationSignal::BarrenStreak, "barren_streak"),
        (DegenerationSignal::RepeatedText, "repeated_text"),
    ];
    for (signal, expected) in pairs {
        let json = serde_json::to_string(&signal).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: DegenerationSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, signal);
    }
}

#[test]
fn gate_close_reason_serde_uses_snake_case() {
    let pairs = [
        (GateCloseReason::ModelRequested, "model_requested"),
        (GateCloseReason::Degeneration, "degeneration"),
        (GateCloseReason::UserSuspend, "user_suspend"),
        (GateCloseReason::IdleTimeout, "idle_timeout"),
    ];
    for (reason, expected) in pairs {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: GateCloseReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, reason);
    }
}

#[test]
fn degeneration_summary_serde_roundtrip() {
    let summary = DegenerationSummary {
        signal: DegenerationSignal::RepeatedText,
        count: 7,
        wake_deadline: Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap(),
    };
    let json = serde_json::to_string(&summary).unwrap();
    let back: DegenerationSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(back.signal, summary.signal);
    assert_eq!(back.count, summary.count);
    assert_eq!(back.wake_deadline, summary.wake_deadline);
}

#[test]
fn continuation_gate_summary_open_omits_optional_fields() {
    let summary = ContinuationGateSummary {
        open: true,
        closed_reason: None,
        wake_deadline: None,
    };
    let json = serde_json::to_string(&summary).unwrap();
    assert_eq!(json, r#"{"open":true}"#);
    let back: ContinuationGateSummary = serde_json::from_str(&json).unwrap();
    assert!(back.open);
    assert!(back.closed_reason.is_none());
    assert!(back.wake_deadline.is_none());
}

#[test]
fn continuation_gate_summary_closed_carries_reason_and_deadline() {
    let when = Utc.with_ymd_and_hms(2026, 5, 21, 13, 30, 0).unwrap();
    let summary = ContinuationGateSummary {
        open: false,
        closed_reason: Some(GateCloseReason::ModelRequested),
        wake_deadline: Some(when),
    };
    let json = serde_json::to_string(&summary).unwrap();
    let back: ContinuationGateSummary = serde_json::from_str(&json).unwrap();
    assert!(!back.open);
    assert_eq!(back.closed_reason, Some(GateCloseReason::ModelRequested));
    assert_eq!(back.wake_deadline, Some(when));
}

#[test]
fn event_payload_degeneration_detected_roundtrip() {
    let payload = AgentEventPayload::DegenerationDetected(DegenerationSummary {
        signal: DegenerationSignal::BarrenStreak,
        count: 20,
        wake_deadline: Utc.with_ymd_and_hms(2026, 5, 21, 14, 0, 0).unwrap(),
    });
    let event = AgentEvent::root(payload);
    let json = serde_json::to_string(&event).unwrap();
    let back: AgentEvent = serde_json::from_str(&json).unwrap();
    match back.payload {
        AgentEventPayload::DegenerationDetected(s) => {
            assert_eq!(s.signal, DegenerationSignal::BarrenStreak);
            assert_eq!(s.count, 20);
        }
        other => panic!("expected DegenerationDetected, got {other:?}"),
    }
}

#[test]
fn event_payload_continuation_gate_changed_roundtrip() {
    let payload = AgentEventPayload::ContinuationGateChanged(ContinuationGateSummary {
        open: false,
        closed_reason: Some(GateCloseReason::UserSuspend),
        wake_deadline: None,
    });
    let event = AgentEvent::root(payload);
    let json = serde_json::to_string(&event).unwrap();
    let back: AgentEvent = serde_json::from_str(&json).unwrap();
    match back.payload {
        AgentEventPayload::ContinuationGateChanged(s) => {
            assert!(!s.open);
            assert_eq!(s.closed_reason, Some(GateCloseReason::UserSuspend));
            assert!(s.wake_deadline.is_none());
        }
        other => panic!("expected ContinuationGateChanged, got {other:?}"),
    }
}
