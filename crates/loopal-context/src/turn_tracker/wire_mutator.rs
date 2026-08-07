use std::time::{Duration, SystemTime};

use loopal_turn::Turn;

use crate::ingestion::condense_server_blocks_in_turns;
use crate::middleware::microcompact::{MicroCompactStats, maybe_microcompact};

// reason: `TurnTracker::with_wire_mut` does not persist a TurnEvent for
// these mutations — they're ephemeral, applied only to the in-memory
// cache for the current process. Replay from turns.jsonl will rebuild
// the pre-mutation turns. Both mutators below MUST be idempotent on
// resume so the next run reaches the same final state without
// triggering hard errors:
//   - `scrub_idle_tool_results` re-scrubs unconditionally when the
//     idle threshold is still exceeded; producing stable output for
//     identical input proves idempotency.
//   - `condense_server_blocks` condenses regardless of prior state;
//     applying it twice produces the same result as once.
// New wire-only mutators added here MUST preserve this invariant or
// route through a persisted-event path instead.

pub fn scrub_idle_tool_results(
    turns: &mut [Turn],
    last_activity: Option<SystemTime>,
    now: SystemTime,
    idle_threshold: Duration,
) -> Option<MicroCompactStats> {
    maybe_microcompact(turns, last_activity, now, idle_threshold)
}

pub fn condense_server_blocks(turns: &mut [Turn]) {
    condense_server_blocks_in_turns(turns);
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_turn::{
        AssistantOutput, ServerBlock, ServerToolCall, ServerToolPair, ServerToolResult, StopReason,
        TextBlock, Turn, TurnStep, TurnTrigger,
    };

    fn turn_with_server_blocks() -> Turn {
        let mut t = Turn::new(TurnTrigger::UserInput {
            envelope_id: "e".into(),
            content: "go".into(),
            images: Vec::new(),
        });
        t.body.steps.push(TurnStep::LlmCall {
            model: "m".into(),
            response: AssistantOutput {
                text_blocks: vec![TextBlock { text: "ok".into() }],
                tool_calls: Vec::new(),
                server_blocks: vec![ServerBlock::ToolPair(ServerToolPair {
                    call: ServerToolCall {
                        id: "s1".into(),
                        name: "web_search".into(),
                        input: serde_json::json!({}),
                    },
                    result: ServerToolResult {
                        block_type: "web_search_tool_result".into(),
                        content: serde_json::json!({"x": 1}),
                    },
                })],
                stop_reason: StopReason::EndTurn,
            },
        });
        t
    }

    #[test]
    fn condense_server_blocks_is_idempotent() {
        let mut turns = vec![turn_with_server_blocks()];
        condense_server_blocks(&mut turns);
        // reason: snapshot full structure after the FIRST condense pass,
        // then re-run and assert deep equality. Comparing only
        // server_blocks.len() would be tautological (both 0 after first
        // call) — a regression that double-appends marker TextBlocks
        // would slip through.
        let after_once = serde_json::to_value(&turns).expect("serialize after_once");
        condense_server_blocks(&mut turns);
        let after_twice = serde_json::to_value(&turns).expect("serialize after_twice");
        assert_eq!(
            after_once, after_twice,
            "condense_server_blocks must be a fixed point (deep equality)"
        );
        // Cross-check: exactly one marker TextBlock per condensed pair.
        let marker_count: usize = turns
            .iter()
            .flat_map(|t| t.body.steps.iter())
            .filter_map(|s| match s {
                TurnStep::LlmCall { response, .. } => Some(
                    response
                        .text_blocks
                        .iter()
                        .filter(|tb| tb.text.contains("result condensed"))
                        .count(),
                ),
                _ => None,
            })
            .sum();
        assert_eq!(
            marker_count, 1,
            "exactly one 'result condensed' marker expected (no duplicates)"
        );
    }

    #[test]
    fn scrub_idle_tool_results_is_idempotent() {
        use loopal_turn::{
            OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, ToolResult,
        };
        let mut t = Turn::new(TurnTrigger::UserInput {
            envelope_id: "e".into(),
            content: "go".into(),
            images: Vec::new(),
        });
        t.body.steps.push(TurnStep::LlmCall {
            model: "m".into(),
            response: AssistantOutput {
                text_blocks: Vec::new(),
                tool_calls: vec![ToolCall {
                    id: ToolCallId::new("c1"),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                }],
                server_blocks: Vec::new(),
                stop_reason: StopReason::ToolUse,
            },
        });
        t.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new("c1"),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                state: ToolExecState::Done(ToolResult {
                    content: "old content".into(),
                    images: Vec::new(),
                    is_error: false,
                    metadata: None,
                }),
            }],
        }));
        let t2 = Turn::new(TurnTrigger::UserInput {
            envelope_id: "e2".into(),
            content: "after".into(),
            images: Vec::new(),
        });
        let mut turns = vec![t, t2];
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let last_activity = Some(SystemTime::UNIX_EPOCH);
        // 60s idle threshold; "10000s ago" easily exceeds.
        let first_stats =
            scrub_idle_tool_results(&mut turns, last_activity, now, Duration::from_secs(60));
        // Baseline: first call must actually scrub at least one tool result.
        // Without this, a future regression that silently no-ops makes the
        // idempotency check vacuously pass on unchanged fixtures.
        let stats = first_stats.expect("first scrub must produce stats");
        assert!(
            stats.results_cleared > 0,
            "first scrub must clear ≥1 tool result; got {}",
            stats.results_cleared
        );
        let after_once = serde_json::to_value(&turns).expect("serialize after_once");
        scrub_idle_tool_results(&mut turns, last_activity, now, Duration::from_secs(60));
        let after_twice = serde_json::to_value(&turns).expect("serialize after_twice");
        assert_eq!(
            after_once, after_twice,
            "scrub_idle_tool_results must be a fixed point (deep equality)"
        );
    }
}
