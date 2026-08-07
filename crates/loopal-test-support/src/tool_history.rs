use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, ToolResult, Turn, TurnOutcome, TurnStep, TurnTrigger,
};

pub struct ToolStep {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub result_body: String,
    pub state: ToolExecState,
}

impl ToolStep {
    pub fn done(tool: &str, id: &str, body: &str) -> Self {
        Self {
            tool_use_id: id.into(),
            tool_name: tool.into(),
            input: serde_json::json!({}),
            result_body: body.into(),
            state: ToolExecState::Done(ToolResult {
                content: body.into(),
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }),
        }
    }

    pub fn done_with_input(tool: &str, id: &str, input: serde_json::Value, body: &str) -> Self {
        let mut step = Self::done(tool, id, body);
        step.input = input;
        step
    }

    pub fn cancelled(tool: &str, id: &str, cause: loopal_turn::CancelCause) -> Self {
        Self {
            tool_use_id: id.into(),
            tool_name: tool.into(),
            input: serde_json::json!({}),
            result_body: String::new(),
            state: ToolExecState::Cancelled(cause),
        }
    }
}

pub fn tool_history_turn(trigger_content: &str, steps: Vec<ToolStep>) -> Turn {
    let mut turn = Turn::new(TurnTrigger::UserInput {
        envelope_id: String::new(),
        content: trigger_content.into(),
        images: Vec::new(),
    });
    let tool_calls: Vec<ToolCall> = steps
        .iter()
        .map(|s| ToolCall {
            id: ToolCallId::new(&s.tool_use_id),
            name: s.tool_name.clone(),
            input: s.input.clone(),
        })
        .collect();
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: tool_calls.clone(),
            server_blocks: vec![],
            stop_reason: StopReason::ToolUse,
        },
    });
    let items: Vec<ToolBatchItem> = steps
        .into_iter()
        .map(|s| ToolBatchItem {
            call: ToolCall {
                id: ToolCallId::new(&s.tool_use_id),
                name: s.tool_name,
                input: s.input,
            },
            state: s.state,
        })
        .collect();
    turn.body
        .steps
        .push(TurnStep::ToolBatch(OrderedToolBatch { items }));
    turn.outcome = TurnOutcome::Complete;
    turn
}

/// Backdate every seeded turn's `last_step_at` so `last_assistant_activity_at`
/// reports the supplied stale instant. Used by microcompact e2e tests to
/// simulate the idle-timeout firing without sleeping wall-clock time.
pub fn backdate_activity(
    runner: &mut loopal_runtime::agent_loop::AgentLoopRunner,
    seconds_ago: i64,
) {
    runner.turns.with_wire_mut(|turns| {
        let stale = chrono::Utc::now() - chrono::Duration::seconds(seconds_ago);
        for t in turns.iter_mut() {
            t.last_step_at = Some(stale);
        }
    });
}

/// Reopen the most recent Complete turn so it accepts further step appends.
/// Used by tests that exercise functions (rehydrate, compaction-summary)
/// which require an InProgress host turn to write into.
pub fn reopen_for_test(runner: &mut loopal_runtime::agent_loop::AgentLoopRunner) {
    runner
        .turns
        .reopen_last_completed_turn()
        .expect("seeded store must have at least one Complete turn to reopen");
}
