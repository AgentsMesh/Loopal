use loopal_protocol::{AgentEventPayload, ResolveSource};
use loopal_view_state::ViewStateReducer;

fn message_count(r: &ViewStateReducer) -> usize {
    r.state().agent.conversation.messages.len()
}

fn apply_decided(r: &mut ViewStateReducer, source: ResolveSource, reason: &str) -> Option<u64> {
    r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 1234,
        reason: reason.into(),
        source,
    })
}

#[test]
fn manual_source_does_not_emit_system_msg() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = apply_decided(&mut r, ResolveSource::Manual, "user picked X");
    assert!(bumped.is_none(), "manual must not bump rev");
    assert_eq!(message_count(&r), before);
}

#[test]
fn classifier_source_does_not_emit_system_msg() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = apply_decided(&mut r, ResolveSource::Classifier, "classifier inferred");
    assert!(bumped.is_none(), "classifier must not bump rev");
    assert_eq!(message_count(&r), before);
}

#[test]
fn agent_source_does_not_emit_system_msg() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = apply_decided(
        &mut r,
        ResolveSource::Agent,
        "sub-agent looked at git status",
    );
    assert!(bumped.is_none(), "agent must not bump rev");
    assert_eq!(message_count(&r), before);
}

#[test]
fn resolve_source_serde_canonical_roundtrip() {
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
