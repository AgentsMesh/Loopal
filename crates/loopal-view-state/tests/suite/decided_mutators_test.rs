use loopal_protocol::{AgentEventPayload, ResolveSource};
use loopal_view_state::ViewStateReducer;

fn last_message_content(r: &ViewStateReducer) -> String {
    r.state()
        .agent
        .conversation
        .messages
        .last()
        .expect("a message should have been pushed")
        .content
        .clone()
}

#[test]
fn permission_decided_allow_with_reason_and_duration() {
    let mut r = ViewStateReducer::new("root");
    let bumped = r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Bash".into(),
        decision: "allow".into(),
        reason: "normal cargo test".into(),
        duration_ms: 42,
    });
    assert!(bumped.is_some(), "permission_decided must bump rev");
    let msg = last_message_content(&r);
    assert!(msg.contains("permission allowed"), "label missing: {msg}");
    assert!(msg.contains("Bash"), "tool name missing: {msg}");
    assert!(msg.contains("normal cargo test"), "reason missing: {msg}");
    assert!(msg.contains("(42ms)"), "duration missing: {msg}");
}

#[test]
fn permission_decided_deny_without_duration() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "rm -rf".into(),
        decision: "deny".into(),
        reason: "dangerous".into(),
        duration_ms: 0,
    });
    let msg = last_message_content(&r);
    assert!(
        msg.contains("permission denied"),
        "deny label missing: {msg}"
    );
    assert!(msg.contains("rm -rf"), "tool name missing: {msg}");
    assert!(
        !msg.contains("(0ms)"),
        "zero duration must be omitted: {msg}"
    );
}

#[test]
fn permission_decided_empty_reason_omits_colon() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Read".into(),
        decision: "allow".into(),
        reason: String::new(),
        duration_ms: 12,
    });
    let msg = last_message_content(&r);
    assert!(
        !msg.contains(": "),
        "no reason → no ': reason' segment, got: {msg}"
    );
    assert!(
        msg.contains("(12ms)"),
        "duration should still appear: {msg}"
    );
    assert!(msg.contains("Read"));
}

#[test]
fn permission_decided_unknown_decision_uses_neutral_label() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Tool".into(),
        decision: "ask".into(),
        reason: "intermediate".into(),
        duration_ms: 0,
    });
    let msg = last_message_content(&r);
    // "ask" or any unknown decision uses the bare "permission" label
    assert!(msg.contains("[permission]"), "neutral label missing: {msg}");
    assert!(!msg.contains("permission allowed"));
    assert!(!msg.contains("permission denied"));
}

#[test]
fn question_decided_with_reason_and_duration() {
    let mut r = ViewStateReducer::new("root");
    let bumped = r.apply(AgentEventPayload::QuestionDecided {
        question_count: 3,
        duration_ms: 150,
        reason: "chose conservative defaults".into(),
        source: ResolveSource::Manual,
    });
    assert!(bumped.is_some(), "question_decided must bump rev");
    let msg = last_message_content(&r);
    assert!(msg.contains("[ask-user] manual"), "label missing: {msg}");
    assert!(
        msg.contains("chose conservative defaults"),
        "reason missing: {msg}"
    );
    assert!(msg.contains("(150ms)"), "duration missing: {msg}");
}

#[test]
fn question_decided_empty_reason_falls_back_to_count() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 0,
        reason: String::new(),
        source: ResolveSource::Manual,
    });
    let msg = last_message_content(&r);
    assert!(msg.contains("1 question(s)"), "count missing: {msg}");
    assert!(!msg.contains("(0ms)"), "zero duration must be omitted");
}

#[test]
fn question_decided_classifier_source_in_label() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 8200,
        reason: "代码探索".into(),
        source: ResolveSource::Classifier,
    });
    let msg = last_message_content(&r);
    assert!(
        msg.contains("[ask-user] classifier"),
        "classifier label missing: {msg}"
    );
    assert!(msg.contains("代码探索"), "answer missing: {msg}");
    assert!(msg.contains("(8200ms)"));
}

#[test]
fn question_decided_agent_source_in_label() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 24500,
        reason: "looked at git status".into(),
        source: ResolveSource::Agent,
    });
    let msg = last_message_content(&r);
    assert!(
        msg.contains("[ask-user] agent"),
        "agent label missing: {msg}"
    );
}
