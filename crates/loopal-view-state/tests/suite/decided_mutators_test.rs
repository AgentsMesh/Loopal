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

fn message_count(r: &ViewStateReducer) -> usize {
    r.state().agent.conversation.messages.len()
}

#[test]
fn permission_decided_allow_is_silent() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Bash".into(),
        decision: "allow".into(),
        reason: "normal cargo test".into(),
        duration_ms: 42,
    });
    assert!(bumped.is_none(), "allow must not bump rev");
    assert_eq!(
        message_count(&r),
        before,
        "allow must not push a system msg"
    );
}

#[test]
fn permission_decided_allow_empty_reason_is_silent() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Read".into(),
        decision: "allow".into(),
        reason: String::new(),
        duration_ms: 0,
    });
    assert!(bumped.is_none(), "allow must not bump rev");
    assert_eq!(message_count(&r), before, "allow must not push msg");
}

#[test]
fn permission_decided_deny_without_duration() {
    let mut r = ViewStateReducer::new("root");
    let bumped = r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "rm -rf".into(),
        decision: "deny".into(),
        reason: "dangerous".into(),
        duration_ms: 0,
    });
    assert!(bumped.is_some(), "deny must bump rev");
    let msg = last_message_content(&r);
    assert!(
        msg.contains("permission denied"),
        "deny label missing: {msg}"
    );
    assert!(msg.contains("rm -rf"), "tool name missing: {msg}");
    assert!(msg.contains("dangerous"), "reason missing: {msg}");
    assert!(
        !msg.contains("(0ms)"),
        "zero duration must be omitted: {msg}"
    );
}

#[test]
fn permission_decided_deny_with_duration() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Bash".into(),
        decision: "deny".into(),
        reason: "policy".into(),
        duration_ms: 1500,
    });
    let msg = last_message_content(&r);
    assert!(msg.contains("permission denied"), "deny label: {msg}");
    assert!(msg.contains("(1500ms)"), "duration must appear: {msg}");
}

#[test]
fn permission_decided_deny_empty_reason_omits_colon() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Edit".into(),
        decision: "deny".into(),
        reason: String::new(),
        duration_ms: 0,
    });
    let msg = last_message_content(&r);
    assert!(
        !msg.contains(": "),
        "no reason → no ': reason' segment, got: {msg}"
    );
    assert!(msg.contains("Edit"));
}

#[test]
fn permission_decided_unknown_decision_uses_neutral_label() {
    let mut r = ViewStateReducer::new("root");
    let bumped = r.apply(AgentEventPayload::PermissionDecided {
        tool_name: "Tool".into(),
        decision: "ask".into(),
        reason: "intermediate".into(),
        duration_ms: 0,
    });
    assert!(
        bumped.is_some(),
        "unknown decision still pushes (diagnostic)"
    );
    let msg = last_message_content(&r);
    assert!(msg.contains("[permission]"), "neutral label missing: {msg}");
    assert!(!msg.contains("permission allowed"));
    assert!(!msg.contains("permission denied"));
}

#[test]
fn question_decided_manual_is_silent() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = r.apply(AgentEventPayload::QuestionDecided {
        question_count: 3,
        duration_ms: 150,
        reason: "chose conservative defaults".into(),
        source: ResolveSource::Manual,
    });
    assert!(bumped.is_none(), "question_decided must not bump rev");
    assert_eq!(message_count(&r), before, "must not push system msg");
}

#[test]
fn question_decided_classifier_is_silent() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 8200,
        reason: "代码探索".into(),
        source: ResolveSource::Classifier,
    });
    assert!(bumped.is_none(), "classifier source must not bump rev");
    assert_eq!(message_count(&r), before);
}

#[test]
fn question_decided_agent_is_silent() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 24500,
        reason: "looked at git status".into(),
        source: ResolveSource::Agent,
    });
    assert!(bumped.is_none(), "agent source must not bump rev");
    assert_eq!(message_count(&r), before);
}

#[test]
fn question_decided_empty_reason_is_silent() {
    let mut r = ViewStateReducer::new("root");
    let before = message_count(&r);
    let bumped = r.apply(AgentEventPayload::QuestionDecided {
        question_count: 1,
        duration_ms: 0,
        reason: String::new(),
        source: ResolveSource::Manual,
    });
    assert!(bumped.is_none());
    assert_eq!(message_count(&r), before);
}
