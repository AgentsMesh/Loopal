use std::time::{Duration, SystemTime};

use loopal_turn::{ToolExecState, Turn, TurnStep};

pub const DEFAULT_IDLE_MINUTES: u64 = 60;
const CLEARED_MARKER: &str = "[Old tool result content cleared after idle timeout]";

const SCRUBBABLE_TOOLS: &[&str] = &[
    "Read",
    "Write",
    "Edit",
    "MultiEdit",
    "Bash",
    "Grep",
    "Glob",
    "WebFetch",
    "WebSearch",
    "Ls",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct MicroCompactStats {
    pub results_cleared: usize,
}

pub(crate) fn maybe_microcompact(
    turns: &mut [Turn],
    last_activity: Option<SystemTime>,
    now: SystemTime,
    idle_threshold: Duration,
) -> Option<MicroCompactStats> {
    let elapsed = match last_activity {
        Some(t) => now.duration_since(t).ok()?,
        None => return None,
    };
    if elapsed < idle_threshold {
        return None;
    }
    Some(scrub_turns_in_place(turns))
}

fn scrub_turns_in_place(turns: &mut [Turn]) -> MicroCompactStats {
    let mut stats = MicroCompactStats::default();
    for turn in turns.iter_mut() {
        for step in &mut turn.body.steps {
            let TurnStep::ToolBatch(batch) = step else {
                continue;
            };
            for item in &mut batch.items {
                if !SCRUBBABLE_TOOLS.contains(&item.call.name.as_str()) {
                    continue;
                }
                let ToolExecState::Done(result) = &mut item.state else {
                    continue;
                };
                if result.content.as_str() == CLEARED_MARKER {
                    continue;
                }
                result.content = CLEARED_MARKER.to_string();
                stats.results_cleared += 1;
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_turn::{
        OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolResult, Turn, TurnStep,
        TurnTrigger,
    };

    fn tool_batch_turn(tool_name: &str, body: &str) -> Turn {
        let mut turn = Turn::new(TurnTrigger::Resume);
        turn.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new("tc-1"),
                    name: tool_name.to_string(),
                    input: serde_json::json!({}),
                },
                state: ToolExecState::Done(ToolResult {
                    content: body.to_string(),
                    images: Vec::new(),
                    is_error: false,
                    metadata: None,
                }),
            }],
        }));
        turn
    }

    fn assert_content(turn: &Turn, expected: &str) {
        let TurnStep::ToolBatch(batch) = &turn.body.steps[0] else {
            panic!("expected ToolBatch step");
        };
        let ToolExecState::Done(r) = &batch.items[0].state else {
            panic!("expected Done state");
        };
        assert_eq!(r.content, expected);
    }

    #[test]
    fn no_op_when_recent() {
        let mut turns = vec![tool_batch_turn("Read", "hello")];
        let result = maybe_microcompact(
            &mut turns,
            Some(SystemTime::now()),
            SystemTime::now(),
            Duration::from_secs(60),
        );
        assert!(result.is_none() || result.unwrap().results_cleared == 0);
    }

    #[test]
    fn scrubs_after_threshold() {
        let mut turns = vec![tool_batch_turn("Read", "hello")];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats = maybe_microcompact(&mut turns, Some(last), now, Duration::from_secs(60))
            .expect("should fire");
        assert_eq!(stats.results_cleared, 1);
        assert_content(&turns[0], CLEARED_MARKER);
    }

    #[test]
    fn idempotent_does_not_recount_cleared() {
        let mut turns = vec![tool_batch_turn("Read", CLEARED_MARKER)];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats =
            maybe_microcompact(&mut turns, Some(last), now, Duration::from_secs(60)).unwrap();
        assert_eq!(stats.results_cleared, 0);
    }

    #[test]
    fn leaves_non_scrubbable_tools_alone() {
        let mut turns = vec![tool_batch_turn("Plan", "deep deliberation")];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats =
            maybe_microcompact(&mut turns, Some(last), now, Duration::from_secs(60)).unwrap();
        assert_eq!(stats.results_cleared, 0);
        assert_content(&turns[0], "deep deliberation");
    }

    #[test]
    fn no_op_when_last_activity_unset() {
        let mut turns = vec![tool_batch_turn("Read", "x")];
        let result =
            maybe_microcompact(&mut turns, None, SystemTime::now(), Duration::from_secs(60));
        assert!(result.is_none());
    }

    #[test]
    fn scrubs_each_recognized_tool() {
        let tools = ["Read", "Write", "Edit", "Bash", "Grep", "Glob", "WebFetch"];
        for t in tools {
            let mut turns = vec![tool_batch_turn(t, "body")];
            let now = SystemTime::now();
            let last = now - Duration::from_secs(120);
            let stats =
                maybe_microcompact(&mut turns, Some(last), now, Duration::from_secs(60)).unwrap();
            assert_eq!(stats.results_cleared, 1, "tool {t} should scrub");
        }
    }

    #[test]
    fn cancelled_tool_state_not_touched() {
        use loopal_turn::CancelCause;
        let mut turn = Turn::new(TurnTrigger::Resume);
        turn.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new("tc-1"),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                state: ToolExecState::Cancelled(CancelCause::UserInterrupt),
            }],
        }));
        let mut turns = vec![turn];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats =
            maybe_microcompact(&mut turns, Some(last), now, Duration::from_secs(60)).unwrap();
        assert_eq!(stats.results_cleared, 0);
    }
}
