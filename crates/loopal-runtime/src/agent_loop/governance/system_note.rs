use loopal_provider_api::MessageOrigin;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{InjectionKind, TurnStep};

use super::super::runner::AgentLoopRunner;
use super::bridge::DataPlaneBridge;

pub fn make_governance_feedback(feedback: &str) -> Option<Message> {
    if feedback.is_empty() {
        return None;
    }
    Some(Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: feedback.to_string(),
        }],
        origin: Some(MessageOrigin::GovernanceFeedback),
        ephemeral_in_history: false,
    })
}

impl AgentLoopRunner {
    fn record_injection_from_message(&mut self, msg: &Message) {
        let text = msg.text_content();
        if text.is_empty() {
            return;
        }
        let kind = match msg.origin {
            Some(MessageOrigin::GovernanceFeedback)
            | Some(MessageOrigin::GovernanceCompensation) => InjectionKind::Governance,
            Some(MessageOrigin::StopFeedback) => InjectionKind::StopFeedback,
            Some(MessageOrigin::ConfigRefresh) => InjectionKind::ConfigRefresh,
            Some(MessageOrigin::Other { .. }) => InjectionKind::SystemNote,
            _ => return,
        };
        if let Err(e) = self.append_step_record(TurnStep::Injection { kind, text }) {
            tracing::warn!(error = %e, "append_step(Injection) failed in governance bridge");
        }
    }
}

impl DataPlaneBridge for AgentLoopRunner {
    fn write_tool_result_stub(&mut self, msg: Message) {
        self.record_injection_from_message(&msg);
    }

    fn push_system_note(&mut self, msg: Message) {
        self.record_injection_from_message(&msg);
    }
}
