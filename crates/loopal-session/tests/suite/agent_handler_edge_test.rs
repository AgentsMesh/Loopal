//! Edge-case tests for agent_handler: RetryError/RetryCleared on sub-agents.

use loopal_protocol::AgentStatus;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

#[test]
fn test_retry_error_keeps_running() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::RetryError {
                message: "502".into(),
                attempt: 1,
                max_attempts: 6,
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}

#[test]
fn test_retry_cleared_no_crash() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::RetryCleared),
    );
    // Status unchanged from Started → Running
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}

#[test]
fn finished_clears_running_agent() {
    let mut state = make_state();
    // Simulate agent in Running state (ThinkingStream sets Running)
    apply_event(
        &mut state,
        AgentEvent::named("sub1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "sub1",
            AgentEventPayload::ThinkingStream { text: "...".into() },
        ),
    );
    assert_eq!(state.agents["sub1"].observable.status, AgentStatus::Running);

    // Simulate disconnect → synthetic Finished event
    apply_event(
        &mut state,
        AgentEvent::named("sub1", AgentEventPayload::Finished),
    );
    assert_eq!(
        state.agents["sub1"].observable.status,
        AgentStatus::Finished
    );
}

#[test]
fn duplicate_finished_is_idempotent() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("sub1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named("sub1", AgentEventPayload::Finished),
    );
    apply_event(
        &mut state,
        AgentEvent::named("sub1", AgentEventPayload::Finished),
    );
    assert_eq!(
        state.agents["sub1"].observable.status,
        AgentStatus::Finished
    );
}

fn last_assistant_content(state: &SessionState, agent: &str) -> String {
    let conv = &state.agents[agent].conversation;
    conv.messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant" || m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default()
}

#[test]
fn auto_mode_decision_allow_with_duration_shows_ms() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::AutoModeDecision {
                tool_name: "Read".into(),
                decision: "allow".into(),
                reason: "read-only".into(),
                duration_ms: 42,
            },
        ),
    );
    let msg = last_assistant_content(&state, "w1");
    assert!(msg.contains("auto-allowed"), "got: {msg}");
    assert!(msg.contains("Read"), "got: {msg}");
    assert!(msg.contains("(42ms)"), "got: {msg}");
}

#[test]
fn auto_mode_decision_deny_with_zero_duration_shows_cached() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::AutoModeDecision {
                tool_name: "Bash".into(),
                decision: "deny".into(),
                reason: "dangerous".into(),
                duration_ms: 0,
            },
        ),
    );
    let msg = last_assistant_content(&state, "w1");
    assert!(msg.contains("auto-denied"), "got: {msg}");
    assert!(msg.contains("(cached)"), "got: {msg}");
}

#[test]
fn auto_mode_decision_allow_cached_combination() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::AutoModeDecision {
                tool_name: "Glob".into(),
                decision: "allow".into(),
                reason: "r".into(),
                duration_ms: 0,
            },
        ),
    );
    let msg = last_assistant_content(&state, "w1");
    assert!(msg.contains("auto-allowed"));
    assert!(msg.contains("(cached)"));
}

#[test]
fn auto_mode_decision_deny_timed_combination() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::AutoModeDecision {
                tool_name: "Write".into(),
                decision: "deny".into(),
                reason: "r".into(),
                duration_ms: 123,
            },
        ),
    );
    let msg = last_assistant_content(&state, "w1");
    assert!(msg.contains("auto-denied"));
    assert!(msg.contains("(123ms)"));
}

#[test]
fn user_question_request_sets_pending_question() {
    use loopal_protocol::Question;
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::UserQuestionRequest {
                id: "q-42".into(),
                questions: vec![Question {
                    question: "Continue?".into(),
                    options: Vec::new(),
                    allow_multiple: false,
                }],
            },
        ),
    );
    let pending = &state.agents["w1"].conversation.pending_question;
    assert!(pending.is_some(), "pending_question must be set");
    let q = pending.as_ref().unwrap();
    assert_eq!(q.id, "q-42");
    assert_eq!(q.questions.len(), 1);
}
