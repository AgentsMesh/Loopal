use loopal_storage::synthesize_missing_tool_batches;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, Turn, TurnStep, TurnTrigger,
};

fn llm_call(tool_id: Option<&str>) -> TurnStep {
    TurnStep::LlmCall {
        model: "fixture".into(),
        response: AssistantOutput {
            text_blocks: Vec::new(),
            tool_calls: tool_id
                .into_iter()
                .map(|id| ToolCall {
                    id: ToolCallId::new(id),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                })
                .collect(),
            server_blocks: Vec::new(),
            stop_reason: StopReason::ToolUse,
        },
    }
}

#[test]
fn scans_nonmatching_intermediate_steps_before_the_tool_batch() {
    let call = ToolCall {
        id: ToolCallId::new("call-one"),
        name: "Read".into(),
        input: serde_json::json!({}),
    };
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps = vec![
        llm_call(Some("call-one")),
        TurnStep::Injection {
            kind: loopal_turn::InjectionKind::SystemNote,
            text: "intermediate".into(),
        },
        llm_call(None),
        TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call,
                state: ToolExecState::Pending,
            }],
        }),
    ];

    synthesize_missing_tool_batches(&mut turn);

    assert_eq!(turn.body.steps.len(), 4);
    assert!(matches!(turn.body.steps[3], TurnStep::ToolBatch(_)));
}
