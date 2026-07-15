use super::*;

fn turn_with_user(content: &str) -> Turn {
    Turn::new(TurnTrigger::UserInput {
        envelope_id: String::new(),
        content: content.into(),
        images: Vec::new(),
    })
}

#[test]
fn boundary_finds_latest_summary_across_turns() {
    let mut t0 = turn_with_user("first user");
    t0.body.steps.push(TurnStep::CompactionSummary(
        loopal_turn::CompactionSummary {
            summary_text: "first-summary".into(),
            ack_text: "first-ack".into(),
            kept_turn_count: 0,
            removed_turn_count: 0,
        },
    ));
    let mut t1 = turn_with_user("second user");
    t1.body.steps.push(TurnStep::CompactionSummary(
        loopal_turn::CompactionSummary {
            summary_text: "second-summary".into(),
            ack_text: "second-ack".into(),
            kept_turn_count: 0,
            removed_turn_count: 0,
        },
    ));
    let turns = vec![t0, t1];
    assert_eq!(find_compaction_drop_index(&turns), 1);
}

#[test]
fn boundary_none_when_no_summary() {
    let turns = vec![turn_with_user("hello")];
    assert_eq!(find_compaction_drop_index(&turns), 0);
}

#[test]
fn boundary_drops_prior_turns_keeps_boundary_turn_intact() {
    let provider = AnthropicProvider::new(String::new());
    let pre_turn = turn_with_user("pre-compaction content");
    let mut boundary_turn = turn_with_user("user's latest question");
    boundary_turn.body.steps.push(TurnStep::CompactionSummary(
        loopal_turn::CompactionSummary {
            summary_text: "compacted-summary".into(),
            ack_text: "compacted-ack".into(),
            kept_turn_count: 0,
            removed_turn_count: 1,
        },
    ));
    let params = ChatParams::new(
        "claude-sonnet-4-20250514".into(),
        vec![pre_turn, boundary_turn],
        String::new(),
    );
    let out = provider.build_messages_json_from_turns(&params);
    let flat: String = out
        .iter()
        .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
        .filter_map(|b| b["text"].as_str().map(str::to_owned))
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        !flat.contains("pre-compaction content"),
        "prior turn must be dropped: {flat}"
    );
    assert!(
        flat.contains("user's latest question"),
        "boundary turn's trigger must survive: {flat}"
    );
    assert!(
        flat.contains("compacted-summary"),
        "summary must appear: {flat}"
    );
    assert!(flat.contains("compacted-ack"), "ack must appear: {flat}");
}

#[test]
fn boundary_last_role_is_user_after_compact() {
    let provider = AnthropicProvider::new(String::new());
    let mut boundary_turn = turn_with_user("user request");
    boundary_turn.body.steps.push(TurnStep::CompactionSummary(
        loopal_turn::CompactionSummary {
            summary_text: "summary".into(),
            ack_text: "ack".into(),
            kept_turn_count: 0,
            removed_turn_count: 0,
        },
    ));
    let params = ChatParams::new(
        "claude-sonnet-4-20250514".into(),
        vec![boundary_turn],
        String::new(),
    );
    let out = provider.build_messages_json_from_turns(&params);
    let last = out.last().expect("at least one message");
    assert_eq!(
        last["role"], "user",
        "wire must end on user message; got {out:?}"
    );
}

#[test]
fn boundary_turn_projects_summary_before_trigger() {
    let provider = AnthropicProvider::new(String::new());
    let mut boundary_turn = turn_with_user("USER_TRIGGER");
    boundary_turn.body.steps.push(TurnStep::CompactionSummary(
        loopal_turn::CompactionSummary {
            summary_text: "SUMMARY_TEXT".into(),
            ack_text: "ACK_TEXT".into(),
            kept_turn_count: 0,
            removed_turn_count: 1,
        },
    ));
    let params = ChatParams::new(
        "claude-sonnet-4-20250514".into(),
        vec![boundary_turn],
        String::new(),
    );
    let out = provider.build_messages_json_from_turns(&params);
    let flat: Vec<String> = out
        .iter()
        .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
        .filter_map(|b| b["text"].as_str().map(str::to_owned))
        .collect();
    let summary_idx = flat.iter().position(|t| t == "SUMMARY_TEXT");
    let ack_idx = flat.iter().position(|t| t == "ACK_TEXT");
    let trigger_idx = flat.iter().position(|t| t == "USER_TRIGGER");
    assert!(summary_idx.is_some(), "summary missing: {flat:?}");
    assert!(ack_idx.is_some(), "ack missing: {flat:?}");
    assert!(trigger_idx.is_some(), "trigger missing: {flat:?}");
    assert!(
        summary_idx < ack_idx && ack_idx < trigger_idx,
        "expected summary < ack < trigger, got: {flat:?}"
    );
}

#[test]
fn user_trigger_projects_text_and_inline_images() {
    let provider = AnthropicProvider::new(String::new());
    let turn = Turn::new(TurnTrigger::UserInput {
        envelope_id: "image-turn".into(),
        content: "inspect".into(),
        images: vec![loopal_tool_invocation::ToolImageBlock::inline(
            "image/png",
            "iVBORw==",
        )],
    });
    let params = ChatParams::new("claude-opus-4-8".into(), vec![turn], String::new());
    let out = provider.build_messages_json_from_turns(&params);
    let content = out[0]["content"].as_array().unwrap();
    assert_eq!(content[0]["text"], "inspect");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["media_type"], "image/png");
    assert_eq!(content[1]["source"]["data"], "iVBORw==");
}
