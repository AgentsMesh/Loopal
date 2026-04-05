//! Tests for load_sub_agent_history — populates agent entry with display data.

use std::sync::Arc;

use loopal_protocol::UserQuestionResponse;
use loopal_protocol::{AgentStatus, ControlCommand, ProjectedMessage, ProjectedToolCall};
use loopal_session::SessionController;
use tokio::sync::mpsc;

fn make_controller() -> SessionController {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    )
}

#[test]
fn test_load_sub_agent_history_creates_agent_entry() {
    let ctrl = make_controller();

    let projected = vec![ProjectedMessage {
        role: "assistant".into(),
        content: "sub-agent response".into(),
        tool_calls: vec![],
        image_count: 0,
    }];
    ctrl.load_sub_agent_history("worker", "sub-sid", Some("main"), Some("gpt-4"), projected);

    let state = ctrl.lock();
    assert!(state.agents.contains_key("worker"));
    let agent = &state.agents["worker"];
    assert_eq!(agent.parent.as_deref(), Some("main"));
    assert_eq!(agent.session_id.as_deref(), Some("sub-sid"));
    assert_eq!(agent.observable.model, "gpt-4");
    assert_eq!(agent.observable.status, AgentStatus::Finished);
    assert!(agent.is_idle());
    assert_eq!(agent.conversation.messages.len(), 1);
    assert_eq!(agent.conversation.messages[0].content, "sub-agent response");
}

#[test]
fn test_load_sub_agent_history_registers_parent_child() {
    let ctrl = make_controller();

    ctrl.load_sub_agent_history("child-agent", "sid-1", Some("main"), None, vec![]);

    let state = ctrl.lock();
    let main = &state.agents["main"];
    assert!(
        main.children.contains(&"child-agent".to_string()),
        "main should list child-agent as child"
    );
}

#[test]
fn test_load_sub_agent_history_no_parent_no_crash() {
    let ctrl = make_controller();

    ctrl.load_sub_agent_history("orphan", "sid-2", None, None, vec![]);

    let state = ctrl.lock();
    assert!(state.agents.contains_key("orphan"));
    assert!(state.agents["orphan"].parent.is_none());
}

#[test]
fn test_load_sub_agent_history_no_duplicate_children() {
    let ctrl = make_controller();

    ctrl.load_sub_agent_history("child", "sid-1", Some("main"), None, vec![]);
    ctrl.load_sub_agent_history("child", "sid-1", Some("main"), None, vec![]);

    let state = ctrl.lock();
    let children = &state.agents["main"].children;
    assert_eq!(
        children.iter().filter(|c| *c == "child").count(),
        1,
        "should not duplicate child entry"
    );
}

#[test]
fn test_load_sub_agent_history_with_tool_calls() {
    let ctrl = make_controller();

    let projected = vec![ProjectedMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![ProjectedToolCall {
            id: "tc1".into(),
            name: "Read".into(),
            input: Some(serde_json::json!({"path": "/tmp"})),
            summary: "Read /tmp".into(),
            result: Some("contents".into()),
            is_error: false,
            metadata: None,
        }],
        image_count: 0,
    }];
    ctrl.load_sub_agent_history("worker", "sid-3", Some("main"), None, projected);

    let state = ctrl.lock();
    let msgs = &state.agents["worker"].conversation.messages;
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].tool_calls.len(), 1);
    assert_eq!(msgs[0].tool_calls[0].name, "Read");
}

#[test]
fn test_load_sub_agent_history_model_optional() {
    let ctrl = make_controller();

    ctrl.load_sub_agent_history("agent", "sid-4", Some("main"), None, vec![]);

    let state = ctrl.lock();
    // Default model should be empty string (from AgentViewState::default())
    let agent = &state.agents["agent"];
    assert!(agent.observable.model.is_empty());
}
