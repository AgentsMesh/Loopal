use loopal_turn::{CompactionSummary, Turn, TurnStep, TurnTrigger};

use super::super::message::{ContentBlock, Message};
use super::{project_turn_to_messages, project_turns_to_messages};

fn turn_with_user(content: &str) -> Turn {
    Turn::new(TurnTrigger::UserInput {
        envelope_id: String::new(),
        content: content.into(),
        images: Vec::new(),
    })
}

fn turn_text_dump(msgs: &[Message]) -> String {
    msgs.iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
fn boundary_drops_prior_turns_keeps_boundary_turn() {
    let pre = turn_with_user("pre-content");
    let mut boundary = turn_with_user("user's latest question");
    boundary
        .body
        .steps
        .push(TurnStep::CompactionSummary(CompactionSummary {
            summary_text: "summary".into(),
            ack_text: "ack".into(),
            kept_turn_count: 0,
            removed_turn_count: 1,
        }));
    let msgs = project_turns_to_messages(&[pre, boundary]);
    let flat = turn_text_dump(&msgs);
    assert!(!flat.contains("pre-content"), "prior turn dropped: {flat}");
    assert!(
        flat.contains("user's latest question"),
        "boundary trigger preserved: {flat}"
    );
    assert!(flat.contains("summary"), "summary projected: {flat}");
    assert!(flat.contains("ack"), "ack projected: {flat}");
}

#[test]
fn no_boundary_means_no_drop() {
    let t = turn_with_user("only-content");
    let msgs = project_turn_to_messages(&t);
    assert_eq!(turn_text_dump(&msgs), "only-content");
}
