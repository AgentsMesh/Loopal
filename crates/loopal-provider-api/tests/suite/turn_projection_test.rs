use loopal_provider_api::MessageOrigin;
use loopal_provider_api::{
    ContentBlock, MessageRole, project_turn_to_messages, project_turns_to_messages,
};
use loopal_tool_invocation::ToolResultMetadata;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, ServerBlock, ServerToolCall, ServerToolPair,
    ServerToolResult, StopReason, TextBlock, ThinkingBlock, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, ToolResult, Turn, TurnBody, TurnStep, TurnTrigger,
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
        images: Vec::new(),
    }
}

fn llm_step(text: &str, calls: Vec<ToolCall>) -> TurnStep {
    TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
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
            metadata: None,
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
fn goal_continuation_trigger_projects_to_user_message() {
    // The continuation skip-gate relies on this: a GoalContinuation turn
    // projects to a User message, so goal_continuation_check (last_role != User)
    // won't re-inject and loop after a skip.
    let t = turn_with(
        TurnTrigger::GoalContinuation {
            envelope_id: "env-g".into(),
            content: "keep going".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 1);
    // The skip-gate keys on view().last_role(); assert last (not first) to pin
    // the actual load-bearing property even if projection grows more messages.
    assert_eq!(msgs.last().unwrap().role, MessageRole::User);
}

#[test]
fn workflow_result_projects_exact_content_and_typed_origin() {
    let t = turn_with(
        TurnTrigger::WorkflowResult {
            session_id: "session".into(),
            run_id: "wrun_one".into(),
            terminal_revision: 9,
            payload_digest: format!("sha256:{}", "1".repeat(64)),
            state: "succeeded".into(),
            content: "durable result".into(),
        },
        vec![],
    );
    let messages = project_turn_to_messages(&t);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].text_content(), "durable result");
    assert!(matches!(
        &messages[0].origin,
        Some(MessageOrigin::WorkflowResult {
            run_id,
            terminal_revision: 9,
            state,
        }) if run_id == "wrun_one" && state == "succeeded"
    ));
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
            model: "m".into(),
            response: AssistantOutput {
                text_blocks: vec![],
                tool_calls: vec![],
                server_blocks: vec![ServerBlock::ToolPair(pair)],
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

// reason: 回归 #190 — reasoning 必须是 content 首块(Anthropic)且紧贴各自的
// web_search_call(OpenAI);text/tool_calls 排在所有 server 块之后。
#[test]
fn reasoning_precedes_each_web_search_and_text_follows() {
    let mk_pair = |id: &str| ServerToolPair {
        call: ServerToolCall {
            id: id.into(),
            name: "web_search".into(),
            input: serde_json::json!({}),
        },
        result: ServerToolResult {
            block_type: "web_search_tool_result".into(),
            content: serde_json::json!({}),
        },
    };
    let mk_reason = |sig: &str| {
        ServerBlock::Reasoning(ThinkingBlock {
            thinking: "r".into(),
            signature: Some(sig.into()),
        })
    };
    let t = turn_with(
        user_trigger("q"),
        vec![TurnStep::LlmCall {
            model: "m".into(),
            response: AssistantOutput {
                text_blocks: vec![TextBlock { text: "ans".into() }],
                tool_calls: vec![],
                server_blocks: vec![
                    mk_reason("rs_1"),
                    ServerBlock::ToolPair(mk_pair("ws_1")),
                    mk_reason("rs_2"),
                    ServerBlock::ToolPair(mk_pair("ws_2")),
                ],
                stop_reason: StopReason::EndTurn,
            },
        }],
    );
    let msgs = project_turn_to_messages(&t);
    let asst = msgs
        .iter()
        .find(|m| m.role == MessageRole::Assistant)
        .unwrap();
    assert!(
        matches!(asst.content[0], ContentBlock::Thinking { .. }),
        "first block must be reasoning (Anthropic content[0] constraint)"
    );
    for (i, b) in asst.content.iter().enumerate() {
        if matches!(b, ContentBlock::ServerToolUse { .. }) {
            assert!(
                matches!(asst.content[i - 1], ContentBlock::Thinking { .. }),
                "web_search_call at idx {i} must be preceded by its reasoning"
            );
        }
    }
    let text_idx = asst
        .content
        .iter()
        .position(|b| matches!(b, ContentBlock::Text { .. }))
        .unwrap();
    let last_server = asst
        .content
        .iter()
        .rposition(|b| matches!(b, ContentBlock::ServerToolResult { .. }))
        .unwrap();
    assert!(text_idx > last_server, "text must follow all server blocks");
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
fn compaction_summary_projects_before_trigger_on_userinput_turn() {
    // Regression: ContextOverflow retry / auto-compact at ReadyToCall append
    // CompactionSummary to a UserInput turn. Without the projection fix,
    // wire order would be [user X, summary, ack] — placing the summary AFTER
    // the user query. Correct order is [summary, ack, user X].
    let t = turn_with(
        TurnTrigger::UserInput {
            envelope_id: "env-1".into(),
            content: "analyze foo.txt".into(),
            images: Vec::new(),
        },
        vec![TurnStep::CompactionSummary(
            loopal_turn::CompactionSummary {
                summary_text: "PRIOR-HISTORY-SUMMARY".into(),
                ack_text: "ACK".into(),
                kept_turn_count: 1,
                removed_turn_count: 4,
            },
        )],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(msgs.len(), 3);
    // 0: summary (user role)
    assert_eq!(msgs[0].role, MessageRole::User);
    assert!(
        msgs[0].text_content().contains("PRIOR-HISTORY-SUMMARY"),
        "expected summary first; got: {}",
        msgs[0].text_content()
    );
    // 1: ack (assistant role)
    assert_eq!(msgs[1].role, MessageRole::Assistant);
    assert!(msgs[1].text_content().contains("ACK"));
    // 2: user X (the trigger)
    assert_eq!(msgs[2].role, MessageRole::User);
    assert!(
        msgs[2].text_content().contains("analyze foo.txt"),
        "trigger must appear AFTER compaction summary on UserInput turn"
    );
}

#[test]
fn compaction_summary_projects_before_other_steps() {
    // CompactionSummary must appear before non-summary steps in the same turn,
    // even when they were appended later (auto-compact mid-turn then LLM step).
    use loopal_turn::{AssistantOutput, StopReason, TextBlock};
    let t = turn_with(
        TurnTrigger::UserInput {
            envelope_id: "env-1".into(),
            content: "hi".into(),
            images: Vec::new(),
        },
        vec![
            TurnStep::CompactionSummary(loopal_turn::CompactionSummary {
                summary_text: "S".into(),
                ack_text: "A".into(),
                kept_turn_count: 1,
                removed_turn_count: 3,
            }),
            TurnStep::LlmCall {
                model: "claude-haiku-4-5".into(),
                response: AssistantOutput {
                    text_blocks: vec![TextBlock {
                        text: "RESP".into(),
                    }],
                    tool_calls: vec![],
                    server_blocks: vec![],
                    stop_reason: StopReason::EndTurn,
                },
            },
        ],
    );
    let msgs = project_turn_to_messages(&t);
    // Expected order: summary (user), ack (assistant), trigger (user), llm response (assistant)
    assert_eq!(msgs.len(), 4);
    assert!(msgs[0].text_content().contains("S"));
    assert!(msgs[1].text_content().contains("A"));
    assert!(msgs[2].text_content().contains("hi"));
    assert!(msgs[3].text_content().contains("RESP"));
}

#[test]
fn injection_governance_maps_to_governance_feedback_origin() {
    let t = turn_with(
        TurnTrigger::Resume,
        vec![TurnStep::Injection {
            kind: loopal_turn::InjectionKind::Governance,
            text: "abort".into(),
        }],
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

#[test]
fn completed_tool_metadata_survives_turn_projection() {
    let call = tool_call("patch", "ApplyPatch");
    let expected = ToolResultMetadata::modified_files(vec![
        "/workspace/a.rs".into(),
        "/workspace/b.rs".into(),
    ]);
    let batch = OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: call.clone(),
            state: ToolExecState::Done(ToolResult {
                content: "partial failure".into(),
                is_error: true,
                images: vec![],
                metadata: Some(expected.clone()),
            }),
        }],
    };
    let turn = turn_with(
        user_trigger("patch"),
        vec![llm_step("", vec![call]), TurnStep::ToolBatch(batch)],
    );

    let messages = project_turn_to_messages(&turn);
    let metadata = messages
        .iter()
        .flat_map(|message| &message.content)
        .find_map(|block| match block {
            ContentBlock::ToolResult { metadata, .. } => metadata.as_ref(),
            _ => None,
        });

    assert_eq!(metadata, Some(&expected));
}

#[test]
fn agent_result_trigger_rewraps_in_agent_result_marker() {
    let t = turn_with(
        TurnTrigger::AgentResult {
            envelope_id: "env-r".into(),
            from: "worker".into(),
            content: "found 3 bugs".into(),
        },
        vec![],
    );
    let msgs = project_turn_to_messages(&t);
    assert_eq!(
        msgs[0].text_content(),
        "<agent-result name=\"worker\">\nfound 3 bugs\n</agent-result>"
    );
    assert!(matches!(
        &msgs[0].origin,
        Some(MessageOrigin::Agent { label }) if label == "worker"
    ));
}
