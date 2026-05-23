use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::MessageOrigin;

use super::touched_files::TouchedFile;

pub(super) fn bare_summary(messages: &[Message], files: &[TouchedFile]) -> String {
    let mut tool_calls: std::collections::HashMap<String, usize> = Default::default();
    let mut user_msgs = 0usize;
    for msg in messages {
        if msg.role == MessageRole::User {
            user_msgs += 1;
        }
        for block in &msg.content {
            if let ContentBlock::ToolUse { name, .. } = block {
                *tool_calls.entry(name.clone()).or_insert(0) += 1;
            }
        }
    }
    let mut text = String::from(
        "## Bare Summary (LLM summarization unavailable)\n\n\
         The structured summary could not be produced — falling back to a \
         deterministic outline of what happened in this segment.\n\n",
    );
    text.push_str(&format!("- User turns: {user_msgs}\n"));
    text.push_str(&format!("- Files touched: {}\n", files.len()));
    if !tool_calls.is_empty() {
        text.push_str("- Tool calls:\n");
        let mut entries: Vec<_> = tool_calls.iter().collect();
        entries.sort();
        for (k, v) in entries {
            text.push_str(&format!("  - {k}: {v}\n"));
        }
    }
    text.push_str("\nRe-read any file under 'Recently Touched Files' before editing.");
    text
}

pub(super) fn build_summary_message(
    summary_text: &str,
    old_count: usize,
    files: &[TouchedFile],
) -> Message {
    let mut body =
        format!("[Working state summary of {old_count} earlier messages]\n\n{summary_text}");
    if !files.is_empty() {
        body.push_str("\n\n## Recently Touched Files\n");
        for tf in files {
            let marker = if tf.mutated { "*" } else { "-" };
            body.push_str(&format!("{marker} {}\n", tf.path));
        }
        body.push_str("\n* = mutated by this agent; re-read before editing.");
    }
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text { text: body }],
        origin: Some(MessageOrigin::CompactionSummary),
        ephemeral_in_history: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_summary_counts_user_turns() {
        let m = vec![
            Message::user("u1"),
            Message::assistant("a1"),
            Message::user("u2"),
        ];
        let out = bare_summary(&m, &[]);
        assert!(out.contains("User turns: 2"));
    }

    #[test]
    fn bare_summary_lists_tool_call_counts() {
        let m = vec![Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::ToolUse {
                    id: "1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::ToolUse {
                    id: "2".into(),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::ToolUse {
                    id: "3".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
            ],
            origin: None,
            ephemeral_in_history: false,
        }];
        let out = bare_summary(&m, &[]);
        assert!(out.contains("Read: 2"));
        assert!(out.contains("Bash: 1"));
    }

    #[test]
    fn bare_summary_omits_tool_section_when_none() {
        let m = vec![Message::user("hi")];
        let out = bare_summary(&m, &[]);
        assert!(!out.contains("Tool calls"));
    }

    #[test]
    fn build_summary_message_includes_old_count() {
        let msg = build_summary_message("BODY", 42, &[]);
        assert!(msg.text_content().contains("42 earlier messages"));
    }

    #[test]
    fn build_summary_message_distinguishes_mutated_files() {
        let files = vec![
            TouchedFile {
                path: "/edited.rs".into(),
                mutated: true,
                last_seen_msg_idx: 0,
            },
            TouchedFile {
                path: "/read_only.rs".into(),
                mutated: false,
                last_seen_msg_idx: 0,
            },
        ];
        let msg = build_summary_message("BODY", 5, &files);
        let text = msg.text_content();
        assert!(text.contains("* /edited.rs"));
        assert!(text.contains("- /read_only.rs"));
    }
}
