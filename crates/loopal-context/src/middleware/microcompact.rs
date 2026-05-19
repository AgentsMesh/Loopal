use std::time::{Duration, SystemTime};

use loopal_message::{ContentBlock, Message};

pub const DEFAULT_IDLE_MINUTES: u64 = 60;
const CLEARED_MARKER: &str = "[Old tool result content cleared after idle timeout]";

/// Tool names whose ToolResult bodies are safe to scrub after idle. These
/// are read-only (or trivially repeatable) operations whose value to the
/// model lies in the *fact of execution*, not the verbatim payload.
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

/// Apply microcompaction to `messages` if the conversation has been idle
/// longer than `idle_threshold`. Returns `Some(stats)` when it ran.
pub fn maybe_microcompact(
    messages: &mut [Message],
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
    Some(scrub_in_place(messages))
}

fn scrub_in_place(messages: &mut [Message]) -> MicroCompactStats {
    let scrubbable: std::collections::HashSet<String> = collect_scrubbable_tool_use_ids(messages);
    let mut stats = MicroCompactStats::default();

    for msg in messages.iter_mut() {
        for block in &mut msg.content {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } = block
                && scrubbable.contains(tool_use_id)
                && content.as_str() != CLEARED_MARKER
            {
                *content = CLEARED_MARKER.to_string();
                stats.results_cleared += 1;
            }
        }
    }
    stats
}

fn collect_scrubbable_tool_use_ids(messages: &[Message]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for msg in messages {
        for block in &msg.content {
            if let ContentBlock::ToolUse { id, name, .. } = block
                && SCRUBBABLE_TOOLS.contains(&name.as_str())
            {
                ids.insert(id.clone());
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_message::MessageRole;

    fn tool_use(id: &str, name: &str) -> Message {
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: id.into(),
                name: name.into(),
                input: serde_json::json!({}),
            }],
            origin: None,
        }
    }

    fn tool_result(id: &str, body: &str) -> Message {
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: id.into(),
                content: body.into(),
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }],
            origin: None,
        }
    }

    #[test]
    fn no_op_when_recent() {
        let mut msgs = vec![tool_use("a", "Read"), tool_result("a", "hello")];
        let result = maybe_microcompact(
            &mut msgs,
            Some(SystemTime::now()),
            SystemTime::now(),
            Duration::from_secs(60),
        );
        assert!(result.is_none() || result.unwrap().results_cleared == 0);
    }

    #[test]
    fn scrubs_after_threshold() {
        let mut msgs = vec![tool_use("a", "Read"), tool_result("a", "hello")];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats = maybe_microcompact(&mut msgs, Some(last), now, Duration::from_secs(60))
            .expect("should fire");
        assert_eq!(stats.results_cleared, 1);
        if let ContentBlock::ToolResult { content, .. } = &msgs[1].content[0] {
            assert_eq!(content, CLEARED_MARKER);
        }
    }

    #[test]
    fn idempotent_does_not_recount_cleared() {
        let mut msgs = vec![tool_use("a", "Read"), tool_result("a", CLEARED_MARKER)];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats =
            maybe_microcompact(&mut msgs, Some(last), now, Duration::from_secs(60)).unwrap();
        assert_eq!(stats.results_cleared, 0);
    }

    #[test]
    fn leaves_non_scrubbable_tools_alone() {
        let mut msgs = vec![tool_use("a", "Plan"), tool_result("a", "deep deliberation")];
        let now = SystemTime::now();
        let last = now - Duration::from_secs(120);
        let stats =
            maybe_microcompact(&mut msgs, Some(last), now, Duration::from_secs(60)).unwrap();
        assert_eq!(stats.results_cleared, 0);
        if let ContentBlock::ToolResult { content, .. } = &msgs[1].content[0] {
            assert_eq!(content, "deep deliberation");
        }
    }

    #[test]
    fn no_op_when_last_activity_unset() {
        let mut msgs = vec![tool_use("a", "Read"), tool_result("a", "x")];
        let result =
            maybe_microcompact(&mut msgs, None, SystemTime::now(), Duration::from_secs(60));
        assert!(result.is_none());
    }

    #[test]
    fn scrubs_each_recognized_tool() {
        let tools = ["Read", "Write", "Edit", "Bash", "Grep", "Glob", "WebFetch"];
        for t in tools {
            let mut msgs = vec![tool_use("id", t), tool_result("id", "body")];
            let now = SystemTime::now();
            let last = now - Duration::from_secs(120);
            let stats =
                maybe_microcompact(&mut msgs, Some(last), now, Duration::from_secs(60)).unwrap();
            assert_eq!(stats.results_cleared, 1, "tool {t} should scrub");
        }
    }
}
