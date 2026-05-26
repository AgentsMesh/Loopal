use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_runtime::agent_loop::governance::{DataPlaneBridge, make_governance_feedback};

#[derive(Default)]
struct CapturingBridge {
    notes: Vec<Message>,
}

impl DataPlaneBridge for CapturingBridge {
    fn push_system_note(&mut self, msg: Message) {
        self.notes.push(msg);
    }
}

#[test]
fn mock_bridge_captures_system_note_dispatch() {
    let mut bridge = CapturingBridge::default();
    let note = make_governance_feedback("stop retrying").unwrap();
    bridge.push_system_note(note);

    assert_eq!(bridge.notes.len(), 1);
    let ContentBlock::Text { text } = &bridge.notes[0].content[0] else {
        panic!("expected Text block");
    };
    assert_eq!(text, "stop retrying");
    assert_eq!(bridge.notes[0].role, MessageRole::User);
    assert_eq!(
        bridge.notes[0].origin,
        Some(loopal_provider_api::MessageOrigin::GovernanceFeedback)
    );
}

#[test]
fn make_governance_feedback_empty_yields_no_note() {
    assert!(make_governance_feedback("").is_none());
}

#[test]
fn make_governance_feedback_carries_multiline_text() {
    let msg = make_governance_feedback("line 1\nline 2\nline 3").unwrap();
    let ContentBlock::Text { text } = &msg.content[0] else {
        panic!("expected Text");
    };
    assert!(text.contains("line 1"));
    assert!(text.contains("line 3"));
    assert_eq!(text.lines().count(), 3);
}
