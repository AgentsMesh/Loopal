//! Test-only fixture helper: synthesize a `Vec<Turn>` from a flat
//! `Vec<Message>` (typical legacy test shape) by reverse-projecting messages
//! into Turn shape. The caller is responsible for persisting these turns
//! through `runner.start_turn_record / append_step_record / end_turn_record`
//! so the turns.jsonl file matches the in-memory store (required by
//! crash-recovery + resume).
//!
//! Lives in `loopal-test-support` (dev-deps only) so production code cannot
//! accidentally introduce a wire-format entry into the SSOT.

use loopal_provider_api::{Message, MessageRole};
use loopal_turn::{
    AssistantOutput, InjectionKind, StopReason, TextBlock, Turn, TurnOutcome, TurnStep, TurnTrigger,
};

pub fn reverse_project_messages_to_turns(messages: Vec<Message>) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    for msg in messages {
        let text = msg.text_content();
        match msg.role {
            MessageRole::User => {
                let mut turn = Turn::new(TurnTrigger::UserInput {
                    envelope_id: String::new(),
                    content: text,
                    images: Vec::new(),
                });
                turn.outcome = TurnOutcome::Complete;
                turns.push(turn);
            }
            MessageRole::Assistant => {
                let step = TurnStep::LlmCall {
                    model: "test".into(),
                    response: AssistantOutput {
                        text_blocks: if text.is_empty() {
                            vec![]
                        } else {
                            vec![TextBlock { text }]
                        },
                        tool_calls: vec![],
                        server_blocks: vec![],
                        stop_reason: StopReason::EndTurn,
                    },
                };
                if let Some(last) = turns.last_mut() {
                    last.body.steps.push(step);
                } else {
                    let mut turn = Turn::new(TurnTrigger::Resume);
                    turn.body.steps.push(step);
                    turn.outcome = TurnOutcome::Complete;
                    turns.push(turn);
                }
            }
            MessageRole::System => {
                let step = TurnStep::Injection {
                    kind: InjectionKind::SystemNote,
                    text,
                };
                if let Some(last) = turns.last_mut() {
                    last.body.steps.push(step);
                } else {
                    let mut turn = Turn::new(TurnTrigger::Resume);
                    turn.body.steps.push(step);
                    turn.outcome = TurnOutcome::Complete;
                    turns.push(turn);
                }
            }
        }
    }
    turns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_becomes_user_input_trigger_turn() {
        let turns = reverse_project_messages_to_turns(vec![Message::user("hello")]);
        assert_eq!(turns.len(), 1);
        assert!(matches!(
            turns[0].trigger,
            TurnTrigger::UserInput { ref content, .. } if content == "hello"
        ));
    }

    #[test]
    fn user_then_assistant_pair_becomes_one_turn() {
        let turns =
            reverse_project_messages_to_turns(vec![Message::user("q"), Message::assistant("a")]);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].body.steps.len(), 1);
        assert!(matches!(turns[0].body.steps[0], TurnStep::LlmCall { .. }));
    }

    #[test]
    fn assistant_first_creates_resume_turn() {
        let turns = reverse_project_messages_to_turns(vec![Message::assistant("response")]);
        assert_eq!(turns.len(), 1);
        assert!(matches!(turns[0].trigger, TurnTrigger::Resume));
    }
}
