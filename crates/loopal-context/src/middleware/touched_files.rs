use loopal_message::{ContentBlock, Message};

/// Tools whose `file_path` (or `path`) argument we consider a touched file.
const FILE_TOOLS: &[&str] = &["Read", "Write", "Edit", "MultiEdit"];

#[derive(Debug, Clone)]
pub struct TouchedFile {
    pub path: String,
    pub mutated: bool,
    pub last_seen_msg_idx: usize,
}

pub fn rank_touched_files(messages: &[Message], top_n: usize) -> Vec<TouchedFile> {
    let mut by_path: std::collections::HashMap<String, TouchedFile> =
        std::collections::HashMap::new();

    for (idx, msg) in messages.iter().enumerate() {
        for block in &msg.content {
            let ContentBlock::ToolUse { name, input, .. } = block else {
                continue;
            };
            if !FILE_TOOLS.contains(&name.as_str()) {
                continue;
            }
            let path = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(|v| v.as_str());
            let Some(path) = path else { continue };
            let mutates = matches!(name.as_str(), "Write" | "Edit" | "MultiEdit");
            let entry = by_path
                .entry(path.to_string())
                .or_insert_with(|| TouchedFile {
                    path: path.to_string(),
                    mutated: false,
                    last_seen_msg_idx: idx,
                });
            entry.mutated = entry.mutated || mutates;
            entry.last_seen_msg_idx = idx;
        }
    }

    let mut all: Vec<TouchedFile> = by_path.into_values().collect();
    all.sort_by(|a, b| {
        b.mutated
            .cmp(&a.mutated)
            .then_with(|| b.last_seen_msg_idx.cmp(&a.last_seen_msg_idx))
    });
    all.truncate(top_n);
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_message::{ContentBlock, MessageRole};

    fn tool_use(name: &str, path: &str) -> Message {
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: format!("{name}-{path}"),
                name: name.to_string(),
                input: serde_json::json!({ "file_path": path }),
            }],
            origin: None,
            ephemeral_in_history: false,
        }
    }

    #[test]
    fn empty_messages_return_empty() {
        let result = rank_touched_files(&[], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn deduplicates_by_path() {
        let messages = vec![
            tool_use("Read", "/a.rs"),
            tool_use("Read", "/a.rs"),
            tool_use("Read", "/b.rs"),
        ];
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn write_outranks_read() {
        let messages = vec![tool_use("Read", "/a.rs"), tool_use("Write", "/b.rs")];
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result[0].path, "/b.rs");
        assert!(result[0].mutated);
    }

    #[test]
    fn later_outranks_earlier_when_same_kind() {
        let messages = vec![tool_use("Read", "/a.rs"), tool_use("Read", "/b.rs")];
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result[0].path, "/b.rs");
    }

    #[test]
    fn path_promoted_to_mutated_once_edited() {
        let messages = vec![tool_use("Read", "/a.rs"), tool_use("Edit", "/a.rs")];
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result.len(), 1);
        assert!(result[0].mutated);
    }

    #[test]
    fn top_n_truncates() {
        let messages: Vec<Message> = (0..10)
            .map(|i| tool_use("Read", &format!("/f{i}.rs")))
            .collect();
        let result = rank_touched_files(&messages, 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn accepts_path_key_in_addition_to_file_path() {
        let mut msg = tool_use("Read", "");
        msg.content = vec![ContentBlock::ToolUse {
            id: "x".into(),
            name: "Read".into(),
            input: serde_json::json!({ "path": "/legacy.rs" }),
        }];
        let result = rank_touched_files(&[msg], 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "/legacy.rs");
    }

    #[test]
    fn ignores_non_file_tools() {
        let messages = vec![tool_use("Bash", "/anything"), tool_use("Grep", "/foo")];
        let result = rank_touched_files(&messages, 5);
        assert!(result.is_empty());
    }
}
