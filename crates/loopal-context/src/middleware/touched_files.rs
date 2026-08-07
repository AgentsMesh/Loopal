use std::collections::HashMap;

use loopal_edit_core::patch_parser::parse_patch;
use loopal_provider_api::{ContentBlock, Message};
use loopal_tool_invocation::ToolResultMetadata;

/// File tools whose terminal result determines whether an input path was
/// actually touched. `ApplyPatch` is handled separately because a single call
/// can mutate multiple paths and can fail after committing a non-empty prefix.
const FILE_TOOLS: &[&str] = &["Read", "Write", "Edit", "MultiEdit", "ApplyPatch"];

#[derive(Debug)]
struct PendingFileTool<'a> {
    name: &'a str,
    input: &'a serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct TouchedFile {
    pub path: String,
    pub mutated: bool,
    pub last_seen_msg_idx: usize,
}

pub fn rank_touched_files(messages: &[Message], top_n: usize) -> Vec<TouchedFile> {
    let mut by_path: HashMap<String, TouchedFile> = HashMap::new();
    let mut pending: HashMap<&str, PendingFileTool<'_>> = HashMap::new();

    for (idx, msg) in messages.iter().enumerate() {
        for block in &msg.content {
            match block {
                ContentBlock::ToolUse { id, name, input }
                    if FILE_TOOLS.contains(&name.as_str()) =>
                {
                    pending.insert(
                        id,
                        PendingFileTool {
                            name: name.as_str(),
                            input,
                        },
                    );
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    is_error,
                    metadata,
                    ..
                } => {
                    let Some(call) = pending.remove(tool_use_id.as_str()) else {
                        continue;
                    };
                    record_completed_tool(&mut by_path, call, *is_error, metadata.as_ref(), idx);
                }
                _ => {}
            }
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

fn record_completed_tool(
    by_path: &mut HashMap<String, TouchedFile>,
    call: PendingFileTool<'_>,
    is_error: bool,
    metadata: Option<&ToolResultMetadata>,
    result_msg_idx: usize,
) {
    if call.name == "ApplyPatch" {
        if let Some(ToolResultMetadata::ModifiedFiles { paths }) = metadata {
            for path in paths {
                record_path(by_path, path, true, result_msg_idx);
            }
            return;
        }

        // Old successful turns predate ModifiedFiles metadata. They remain
        // recoverable from the canonical patch AST because every operation
        // completed. An old failed turn is intentionally not guessed: it may
        // have committed any prefix of the document.
        if is_error {
            return;
        }
        let Some(patch) = call.input.get("patch").and_then(|value| value.as_str()) else {
            return;
        };
        let Ok(ops) = parse_patch(patch) else {
            return;
        };
        for op in ops {
            record_path(by_path, &op.path().to_string_lossy(), true, result_msg_idx);
        }
        return;
    }

    if is_error {
        return;
    }
    let Some(path) = call
        .input
        .get("file_path")
        .or_else(|| call.input.get("path"))
        .and_then(|value| value.as_str())
    else {
        return;
    };
    let mutated = matches!(call.name, "Write" | "Edit" | "MultiEdit");
    record_path(by_path, path, mutated, result_msg_idx);
}

fn record_path(
    by_path: &mut HashMap<String, TouchedFile>,
    path: &str,
    mutated: bool,
    result_msg_idx: usize,
) {
    if path.trim().is_empty() {
        return;
    }
    let entry = by_path
        .entry(path.to_string())
        .or_insert_with(|| TouchedFile {
            path: path.to_string(),
            mutated: false,
            last_seen_msg_idx: result_msg_idx,
        });
    entry.mutated |= mutated;
    entry.last_seen_msg_idx = result_msg_idx;
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_provider_api::{ContentBlock, MessageRole};

    fn tool_exchange(name: &str, input: serde_json::Value, is_error: bool) -> Vec<Message> {
        tool_exchange_with_metadata(name, input, is_error, None)
    }

    fn tool_exchange_with_metadata(
        name: &str,
        input: serde_json::Value,
        is_error: bool,
        metadata: Option<ToolResultMetadata>,
    ) -> Vec<Message> {
        let id = format!("{name}-call");
        vec![
            Message {
                id: None,
                role: MessageRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.to_string(),
                    input,
                }],
                origin: None,
                ephemeral_in_history: false,
            },
            Message {
                id: None,
                role: MessageRole::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: id,
                    content: if is_error { "failed" } else { "ok" }.into(),
                    images: Vec::new(),
                    is_error,
                    metadata,
                }],
                origin: None,
                ephemeral_in_history: false,
            },
        ]
    }

    fn tool_exchange_for_path(name: &str, path: &str) -> Vec<Message> {
        tool_exchange(name, serde_json::json!({ "file_path": path }), false)
    }

    fn flatten(exchanges: impl IntoIterator<Item = Vec<Message>>) -> Vec<Message> {
        exchanges.into_iter().flatten().collect()
    }

    fn pending_tool_use(name: &str, path: &str) -> Message {
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
        let messages = flatten([
            tool_exchange_for_path("Read", "/a.rs"),
            tool_exchange_for_path("Read", "/a.rs"),
            tool_exchange_for_path("Read", "/b.rs"),
        ]);
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn write_outranks_read() {
        let messages = flatten([
            tool_exchange_for_path("Read", "/a.rs"),
            tool_exchange_for_path("Write", "/b.rs"),
        ]);
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result[0].path, "/b.rs");
        assert!(result[0].mutated);
    }

    #[test]
    fn later_outranks_earlier_when_same_kind() {
        let messages = flatten([
            tool_exchange_for_path("Read", "/a.rs"),
            tool_exchange_for_path("Read", "/b.rs"),
        ]);
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result[0].path, "/b.rs");
    }

    #[test]
    fn path_promoted_to_mutated_once_edited() {
        let messages = flatten([
            tool_exchange_for_path("Read", "/a.rs"),
            tool_exchange_for_path("Edit", "/a.rs"),
        ]);
        let result = rank_touched_files(&messages, 10);
        assert_eq!(result.len(), 1);
        assert!(result[0].mutated);
    }

    #[test]
    fn top_n_truncates() {
        let messages: Vec<Message> = (0..10)
            .flat_map(|i| tool_exchange_for_path("Read", &format!("/f{i}.rs")))
            .collect();
        let result = rank_touched_files(&messages, 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn accepts_path_key_in_addition_to_file_path() {
        let messages = tool_exchange("Read", serde_json::json!({ "path": "/legacy.rs" }), false);
        let result = rank_touched_files(&messages, 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "/legacy.rs");
    }

    #[test]
    fn ignores_non_file_tools() {
        let messages = flatten([
            tool_exchange_for_path("Bash", "/anything"),
            tool_exchange_for_path("Grep", "/foo"),
        ]);
        let result = rank_touched_files(&messages, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_file_tool_without_terminal_result() {
        let result = rank_touched_files(&[pending_tool_use("Write", "/not-written.rs")], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_failed_single_file_tool() {
        let messages = tool_exchange(
            "Write",
            serde_json::json!({ "file_path": "/not-written.rs" }),
            true,
        );
        assert!(rank_touched_files(&messages, 5).is_empty());
    }

    #[test]
    fn successful_legacy_apply_patch_uses_canonical_parser() {
        let messages = tool_exchange(
            "ApplyPatch",
            serde_json::json!({
                "patch": "*** Begin Patch\n*** Add File: a.txt\n+alpha\n*** Update File: b.txt\n@@\n-old\n+new\n*** End Patch"
            }),
            false,
        );
        let files = rank_touched_files(&messages, 5);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| file.mutated));
        assert!(files.iter().any(|file| file.path == "a.txt"));
        assert!(files.iter().any(|file| file.path == "b.txt"));
    }

    #[test]
    fn partial_apply_patch_uses_structured_applied_paths_even_on_error() {
        let messages = tool_exchange_with_metadata(
            "ApplyPatch",
            serde_json::json!({
                "patch": "*** Begin Patch\n*** Add File: a.txt\n+alpha\n*** Add File: b.txt\n+beta\n*** Add File: never.txt\n+nope\n*** End Patch"
            }),
            true,
            Some(ToolResultMetadata::modified_files(vec![
                "a.txt".into(),
                "b.txt".into(),
            ])),
        );
        let files = rank_touched_files(&messages, 5);
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|file| file.path == "a.txt"));
        assert!(files.iter().any(|file| file.path == "b.txt"));
        assert!(!files.iter().any(|file| file.path == "never.txt"));
    }

    #[test]
    fn failed_legacy_apply_patch_without_metadata_does_not_guess_prefix() {
        let messages = tool_exchange(
            "ApplyPatch",
            serde_json::json!({
                "patch": "*** Begin Patch\n*** Add File: unknown.txt\n+maybe\n*** End Patch"
            }),
            true,
        );
        assert!(rank_touched_files(&messages, 5).is_empty());
    }
}
