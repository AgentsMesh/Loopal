//! Pre-built model response scenarios for integration tests.
//!
//! Each function returns a `Calls` (Vec of turn chunks) ready for
//! `HarnessBuilder::calls()`. Eliminates per-test inline fixture construction.

use loopal_error::LoopalError;
use loopal_provider_api::StreamChunk;
use serde_json::Value;

use crate::chunks;

/// Type alias for mock provider call sequences.
pub type Calls = Vec<Vec<Result<StreamChunk, LoopalError>>>;

/// Single text response turn.
pub fn simple_text(response: &str) -> Calls {
    vec![chunks::text_turn(response)]
}

/// Tool call (one turn) -> text summary (next turn).
pub fn tool_then_text(id: &str, tool_name: &str, input: Value, summary: &str) -> Calls {
    vec![
        chunks::tool_turn(id, tool_name, input),
        chunks::text_turn(summary),
    ]
}

/// Multiple sequential tool calls, each in its own turn, then final text.
pub fn sequential_tools(tools: &[(&str, &str, Value)], summary: &str) -> Calls {
    let mut calls: Calls = tools
        .iter()
        .map(|(id, name, input)| chunks::tool_turn(id, name, input.clone()))
        .collect();
    calls.push(chunks::text_turn(summary));
    calls
}

/// N parallel tool calls in a single turn, then text.
pub fn parallel_tools(tools: &[(&str, &str, Value)], summary: &str) -> Calls {
    let mut turn: Vec<Result<StreamChunk, LoopalError>> = tools
        .iter()
        .map(|(id, name, input)| chunks::tool_use(id, name, input.clone()))
        .collect();
    turn.push(chunks::usage(10, 5));
    turn.push(chunks::done());
    vec![turn, chunks::text_turn(summary)]
}

/// Two-turn interactive conversation (for use with `interactive(true)`).
pub fn two_turn(resp1: &str, resp2: &str) -> Calls {
    vec![chunks::text_turn(resp1), chunks::text_turn(resp2)]
}

/// N-turn interactive conversation.
pub fn n_turn(responses: &[&str]) -> Calls {
    responses.iter().map(|r| chunks::text_turn(r)).collect()
}

/// Thinking block followed by text response.
pub fn thinking_then_text(thinking: &str, response: &str) -> Calls {
    vec![vec![
        chunks::thinking(thinking),
        chunks::thinking_signature("sig"),
        chunks::text(response),
        chunks::usage(10, 5),
        chunks::done(),
    ]]
}

/// Provider error after partial text.
pub fn error_mid_stream(partial: &str, error_msg: &str) -> Calls {
    vec![vec![
        chunks::text(partial),
        chunks::provider_error(error_msg),
    ]]
}

/// Provider error immediately (no partial text).
pub fn immediate_error(error_msg: &str) -> Calls {
    vec![vec![chunks::provider_error(error_msg)]]
}

/// Task lifecycle: TaskCreate then TaskList, then text summary.
pub fn task_lifecycle(subject: &str) -> Calls {
    vec![
        chunks::tool_turn(
            "tc-create",
            "TaskCreate",
            serde_json::json!({ "subject": subject, "description": "test task" }),
        ),
        chunks::tool_turn("tc-list", "TaskList", serde_json::json!({})),
        chunks::text_turn("Tasks managed."),
    ]
}

/// AttemptCompletion tool call.
pub fn attempt_completion(result: &str) -> Calls {
    vec![chunks::tool_turn(
        "tc-done",
        "AttemptCompletion",
        serde_json::json!({ "result": result }),
    )]
}

/// Auto-continuation scenario: first call ends with MaxTokens, second completes.
pub fn auto_continuation(partial: &str, continuation: &str) -> Calls {
    vec![
        vec![
            chunks::text(partial),
            chunks::usage(10, 50),
            chunks::done_max_tokens(),
        ],
        vec![
            chunks::text(continuation),
            chunks::usage(10, 20),
            chunks::done(),
        ],
    ]
}
