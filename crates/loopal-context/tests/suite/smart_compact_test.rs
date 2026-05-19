use loopal_context::middleware::touched_files::rank_touched_files;
use loopal_message::{ContentBlock, Message, MessageRole};

// `compact_to_boundary` requires a live Provider, so its happy-path is exercised
// in runtime integration tests. Here we cover the deterministic helpers it uses.

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
    }
}

#[test]
fn touched_files_includes_only_file_tools() {
    let messages = vec![
        tool_use("Read", "/a.rs"),
        tool_use("Bash", "/ignored"),
        tool_use("Write", "/b.rs"),
    ];
    let files = rank_touched_files(&messages, 10);
    let paths: Vec<&str> = files.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"/b.rs"));
    assert!(paths.contains(&"/a.rs"));
}

#[test]
fn touched_files_mutations_promoted_to_top() {
    let messages = vec![
        tool_use("Read", "/x.rs"),
        tool_use("Read", "/y.rs"),
        tool_use("Write", "/z.rs"),
    ];
    let files = rank_touched_files(&messages, 10);
    assert_eq!(files[0].path, "/z.rs");
    assert!(files[0].mutated);
}
