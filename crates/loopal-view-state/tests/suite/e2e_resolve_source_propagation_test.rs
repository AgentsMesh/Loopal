// E2E: ResolveSource (protocol enum) → AgentEventPayload::QuestionDecided
// → view-state mutator → conversation system message string.
// Locks the wire-to-UI contract for each of the three sources.

use loopal_protocol::{AgentEventPayload, ResolveSource};
use loopal_view_state::ViewStateReducer;

fn last_system_line(r: &ViewStateReducer) -> String {
    r.state()
        .agent
        .conversation
        .messages
        .last()
        .expect("a message should have been pushed")
        .content
        .clone()
}

fn apply_decided(r: &mut ViewStateReducer, source: ResolveSource, reason: &str) {
    r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 1234,
        reason: reason.into(),
        source,
    });
}

#[test]
fn manual_source_propagates_to_system_msg() {
    let mut r = ViewStateReducer::new("root");
    apply_decided(&mut r, ResolveSource::Manual, "user picked X");
    let msg = last_system_line(&r);
    assert!(
        msg.contains("[ask-user] manual"),
        "manual label missing: {msg}"
    );
    assert!(msg.contains("user picked X"), "reason missing: {msg}");
    assert!(msg.contains("(1234ms)"), "duration missing: {msg}");
}

#[test]
fn classifier_source_propagates_to_system_msg() {
    let mut r = ViewStateReducer::new("root");
    apply_decided(&mut r, ResolveSource::Classifier, "classifier inferred");
    let msg = last_system_line(&r);
    assert!(
        msg.contains("[ask-user] classifier"),
        "classifier label missing: {msg}"
    );
    assert!(msg.contains("classifier inferred"));
}

#[test]
fn agent_source_propagates_to_system_msg() {
    let mut r = ViewStateReducer::new("root");
    apply_decided(
        &mut r,
        ResolveSource::Agent,
        "sub-agent looked at git status",
    );
    let msg = last_system_line(&r);
    assert!(
        msg.contains("[ask-user] agent"),
        "agent label missing: {msg}"
    );
    assert!(msg.contains("sub-agent looked at git status"));
}

#[test]
fn resolve_source_serde_canonical_roundtrip() {
    // Round-trip via JSON to lock the wire encoding for each variant.
    for (variant, expected) in [
        (ResolveSource::Manual, "\"manual\""),
        (ResolveSource::Classifier, "\"classifier\""),
        (ResolveSource::Agent, "\"agent\""),
    ] {
        let s = serde_json::to_string(&variant).unwrap();
        assert_eq!(s, expected, "{variant:?} should serialize as {expected}");
        let parsed: ResolveSource = serde_json::from_str(expected).unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn resolve_source_rejects_legacy_auto_string() {
    let parsed = serde_json::from_str::<ResolveSource>("\"auto\"");
    assert!(
        parsed.is_err(),
        "auto must NOT deserialize after alias removal"
    );
}
