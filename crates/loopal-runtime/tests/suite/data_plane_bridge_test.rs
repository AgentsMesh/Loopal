//! Sentinel for the `DataPlaneBridge` abstraction: a non-runner backend
//! can satisfy the trait, which is the whole point of having a trait
//! instead of inlined calls on `AgentLoopRunner`.

use loopal_message::{ContentBlock, Message, MessageOrigin, MessageRole};
use loopal_runtime::agent_loop::governance::{
    DataPlaneBridge, make_governance_feedback, synthesize_aborted_tool_results,
};

#[derive(Default)]
struct CapturingBridge {
    stubs: Vec<Message>,
    notes: Vec<Message>,
}

impl DataPlaneBridge for CapturingBridge {
    fn write_tool_result_stub(&mut self, msg: Message) {
        self.stubs.push(msg);
    }
    fn push_system_note(&mut self, msg: Message) {
        self.notes.push(msg);
    }
}

#[test]
fn mock_bridge_captures_stub_and_note() {
    let mut bridge = CapturingBridge::default();
    let stub = synthesize_aborted_tool_results(
        &[("u1".into(), "Bash".into(), serde_json::json!({}))],
        "loop detected",
    )
    .unwrap();
    bridge.write_tool_result_stub(stub);

    let note = make_governance_feedback("stop retrying").unwrap();
    bridge.push_system_note(note);

    assert_eq!(bridge.stubs.len(), 1);
    assert_eq!(bridge.notes.len(), 1);

    let ContentBlock::ToolResult {
        tool_use_id,
        is_error,
        ..
    } = &bridge.stubs[0].content[0]
    else {
        panic!("expected ToolResult");
    };
    assert_eq!(tool_use_id, "u1");
    assert!(is_error);
    assert_eq!(
        bridge.stubs[0].origin,
        Some(MessageOrigin::GovernanceCompensation)
    );

    let ContentBlock::Text { text } = &bridge.notes[0].content[0] else {
        panic!("expected Text");
    };
    assert_eq!(text, "stop retrying");
    assert_eq!(bridge.notes[0].role, MessageRole::User);
    assert_eq!(
        bridge.notes[0].origin,
        Some(MessageOrigin::GovernanceFeedback)
    );
}

#[test]
fn empty_feedback_yields_no_note() {
    // Builders return Option so callers can skip empty-feedback aborts
    // without per-call emptiness checks.
    assert!(make_governance_feedback("").is_none());
}

#[test]
fn empty_tool_uses_yields_no_stub() {
    assert!(synthesize_aborted_tool_results(&[], "reason").is_none());
}
