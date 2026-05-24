use loopal_provider_api::{
    ContentBlock, MessageRole, project_turn_to_messages, project_turns_to_messages,
};
use loopal_turn::MessageOrigin;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, ServerToolCall, ServerToolPair, ServerToolResult,
    StopReason, TextBlock, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, ToolResult, Turn,
    TurnBody, TurnStep, TurnTrigger,
};

fn turn_with(trigger: TurnTrigger, steps: Vec<TurnStep>) -> Turn {
    let mut t = Turn::new(trigger);
    t.body = TurnBody { steps };
    t
}

fn user_trigger(content: &str) -> TurnTrigger {
    TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: content.into(),
    }
}

fn llm_step(text: &str, calls: Vec<ToolCall>) -> TurnStep {
    TurnStep::LlmCall {
        request_snapshot: loopal_turn::LlmRequestSnapshot {
            model: "m".into(),
            max_tokens: 1,
            tool_count: calls.len() as u32,
            message_count: 0,
        },
        response: AssistantOutput {
            thinking: None,
            text_blocks: if text.is_empty() {
                vec![]
            } else {
                vec![TextBlock { text: text.into() }]
            },
            tool_calls: calls,
            server_blocks: vec![],
            stop_reason: StopReason::EndTurn,
        },
    }
}

fn tool_call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: ToolCallId::new(id),
        name: name.into(),
        input: serde_json::json!({}),
    }
}

fn done_item(call: ToolCall, body: &str) -> ToolBatchItem {
    ToolBatchItem {
        call,
        state: ToolExecState::Done(ToolResult {
            content: body.into(),
            is_error: false,
            images: vec![],
        }),
    }
}

#[test]
fn empty_turns_produce_no_messages() {
    let msgs = project_turns_to_messages(&[]);
    assert!(msgs.is_empty());
}

#[test]
fn user_trigger_emits_human_user_message() {
    let t = turn_with(user_trigger("hello"), vec![]);
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].role, MessageRole::User);
    assert_eq!(msgs[0].text_content(), "hello");
    assert!(matches!(msgs[0].origin, Some(MessageOrigin::Human)));
}

#[test]
fn cron_trigger_prefixed_with_scheduled() {
    let t = turn_with(
        TurnTrigger::Cron {
            envelope_id: "env-2".into(),
            content: "tick body".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].text_content().starts_with("[scheduled] tick body"));
    assert!(matches!(msgs[0].origin, Some(MessageOrigin::Scheduled)));
}

#[test]
fn agent_trigger_prefixed_with_from_address() {
    let t = turn_with(
        TurnTrigger::Agent {
            envelope_id: "env-3".into(),
            from: "hub-a/worker".into(),
            content: "hi".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs[0].text_content(), "[from: hub-a/worker] hi");
    assert!(matches!(
        &msgs[0].origin,
        Some(MessageOrigin::Agent { label }) if label == "hub-a/worker"
    ));
}

#[test]
fn channel_trigger_prefixed_with_channel_and_from() {
    let t = turn_with(
        TurnTrigger::Channel {
            envelope_id: "env-4".into(),
            channel: "general".into(),
            from: "alice".into(),
            content: "hello team".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs[0].text_content(), "[from: #general/alice] hello team");
}

#[test]
fn goal_continuation_emits_visible_user_message() {
    // regression: previously this trigger was projected to None, dropping the
    // injected continuation prompt entirely from LLM context.
    let t = turn_with(
        TurnTrigger::GoalContinuation {
            envelope_id: "env-5".into(),
            content: "continue goal X".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text_content(), "continue goal X");
    assert!(matches!(
        msgs[0].origin,
        Some(MessageOrigin::GoalContinuation)
    ));
}

#[test]
fn background_hook_emits_visible_user_message_with_kind_origin() {
    let t = turn_with(
        TurnTrigger::BackgroundHook {
            envelope_id: "env-6".into(),
            hook_kind: "stop_feedback".into(),
            content: "user requested stop".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text_content(), "user requested stop");
    assert!(matches!(
        &msgs[0].origin,
        Some(MessageOrigin::Other { label }) if label == "stop_feedback"
    ));
}

#[test]
fn llm_text_only_emits_assistant_message() {
    let t = turn_with(user_trigger("hi"), vec![llm_step("hello back", vec![])]);
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[1].role, MessageRole::Assistant);
    assert!(msgs[1].text_content().contains("hello back"));
}

#[test]
fn parallel_tool_ordering_preserved_in_projection() {
    let calls = vec![
        tool_call("a", "Read"),
        tool_call("b", "Read"),
        tool_call("c", "Bash"),
    ];
    let batch = OrderedToolBatch {
        items: vec![
            done_item(calls[0].clone(), "A"),
            done_item(calls[1].clone(), "B"),
            done_item(calls[2].clone(), "C"),
        ],
    };
    let t = turn_with(
        user_trigger("trigger"),
        vec![llm_step("", calls.clone()), TurnStep::ToolBatch(batch)],
    );
    let msgs = project_turn_to_messages(&t);
    let assistant = msgs
        .iter()
        .find(|m| m.role == MessageRole::Assistant)
        .unwrap();
    let user_results = msgs.iter().rfind(|m| m.role == MessageRole::User).unwrap();

    let use_ids: Vec<&str> = assistant
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let result_ids: Vec<&str> = user_results
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(use_ids, vec!["a", "b", "c"]);
    assert_eq!(result_ids, vec!["a", "b", "c"]);
}

#[test]
fn server_tool_pair_emits_use_and_result() {
    let pair = ServerToolPair {
        call: ServerToolCall {
            id: "s1".into(),
            name: "web_search".into(),
            input: serde_json::json!({}),
        },
        result: ServerToolResult {
            block_type: "web_search_tool_result".into(),
            content: serde_json::json!({"hits": []}),
        },
    };
    let t = turn_with(
        user_trigger("q"),
        vec![TurnStep::LlmCall {
            request_snapshot: loopal_turn::LlmRequestSnapshot {
                model: "m".into(),
                max_tokens: 1,
                tool_count: 0,
                message_count: 0,
            },
            response: AssistantOutput {
                thinking: None,
                text_blocks: vec![],
                tool_calls: vec![],
                server_blocks: vec![pair],
                stop_reason: StopReason::EndTurn,
            },
        }],
    );
    let msgs = project_turn_to_messages(&t);
    let asst = msgs
        .iter()
        .find(|m| m.role == MessageRole::Assistant)
        .unwrap();
    let has_use = asst
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::ServerToolUse { .. }));
    let has_result = asst
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::ServerToolResult { .. }));
    assert!(has_use && has_result);
}

#[test]
fn compaction_emits_user_summary_then_assistant_ack() {
    let t = turn_with(
        TurnTrigger::Resume,
        vec![TurnStep::CompactionSummary(
            loopal_turn::CompactionSummary {
                summary_text: "SUM".into(),
                ack_text: "OK".into(),
                kept_turn_count: 0,
                removed_turn_count: 1,
            },
        )],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, MessageRole::User);
    assert!(msgs[0].text_content().contains("SUM"));
    assert_eq!(msgs[1].role, MessageRole::Assistant);
    assert!(msgs[1].text_content().contains("OK"));
}

#[test]
fn injection_governance_maps_to_governance_feedback_origin() {
    let t = turn_with(
        TurnTrigger::Resume,
        vec![TurnStep::Injection(loopal_turn::InjectedMessage {
            kind: loopal_turn::InjectionKind::Governance,
            text: "abort".into(),
        })],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 1);
    assert!(matches!(
        msgs[0].origin,
        Some(MessageOrigin::GovernanceFeedback)
    ));
}

#[test]
fn cancelled_item_produces_error_tool_result() {
    let call = tool_call("x", "Read");
    let batch = OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: call.clone(),
            state: ToolExecState::Cancelled(loopal_turn::CancelCause::UserInterrupt),
        }],
    };
    let t = turn_with(
        user_trigger("t"),
        vec![llm_step("", vec![call]), TurnStep::ToolBatch(batch)],
    );
    let msgs = project_turn_to_messages(&t);
    let user_results = msgs.iter().rfind(|m| m.role == MessageRole::User).unwrap();
    let block = user_results
        .content
        .iter()
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .unwrap();
    assert_eq!(block.0, "Cancelled");
    assert!(block.1);
}
