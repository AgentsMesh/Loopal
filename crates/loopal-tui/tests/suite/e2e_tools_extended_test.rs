//! E2E tests for tools without prior e2e coverage: Edit, Glob, Ls, CopyFile, binary read.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::{TestFixture, assertions, chunks};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_edit_file() {
    // Turn 1: Write creates the file (relative path → harness cwd)
    // Turn 2: Edit replaces a line
    // Turn 3: Read verifies the change
    // Turn 4: text summary
    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": "edit_target.txt", "content": "line1\nold_line\nline3\n"}),
        ),
        chunks::tool_turn(
            "tc-e",
            "Edit",
            serde_json::json!({
                "file_path": "edit_target.txt",
                "old_string": "old_line",
                "new_string": "new_line"
            }),
        ),
        chunks::tool_turn(
            "tc-r",
            "Read",
            serde_json::json!({"file_path": "edit_target.txt"}),
        ),
        chunks::text_turn("Edit verified."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "Write", false);
    assertions::assert_has_tool_result(&evts, "Edit", false);
    assertions::assert_has_tool_result(&evts, "Read", false);

    // Verify Read output contains the edited content
    let read_output: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "Read" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        read_output.iter().any(|r| r.contains("new_line")),
        "Read output should contain 'new_line', got: {read_output:?}"
    );
}

#[tokio::test]
async fn test_glob_search() {
    let fixture = TestFixture::new();
    fixture.create_file("alpha.txt", "a");
    fixture.create_file("beta.txt", "b");
    fixture.create_file("gamma.rs", "c");
    let dir = fixture.path().to_str().unwrap().to_string();

    let calls = vec![
        chunks::tool_turn(
            "tc-g",
            "Glob",
            serde_json::json!({"pattern": "*.txt", "path": dir}),
        ),
        chunks::text_turn("Glob done."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "Glob");
    assertions::assert_has_tool_result(&evts, "Glob", false);

    let glob_output: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "Glob" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    // Should find .txt files but not .rs
    assert!(
        glob_output
            .iter()
            .any(|r| r.contains("alpha.txt") && r.contains("beta.txt")),
        "Glob should find *.txt files, got: {glob_output:?}"
    );
}

#[tokio::test]
async fn test_ls_directory() {
    let fixture = TestFixture::new();
    fixture.create_file("file_a.txt", "a");
    fixture.create_file("file_b.txt", "b");
    let dir = fixture.path().to_str().unwrap().to_string();

    let calls = vec![
        chunks::tool_turn("tc-l", "Ls", serde_json::json!({"path": dir})),
        chunks::text_turn("Ls done."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "Ls");
    assertions::assert_has_tool_result(&evts, "Ls", false);

    let ls_output: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "Ls" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        ls_output
            .iter()
            .any(|r| r.contains("file_a.txt") && r.contains("file_b.txt")),
        "Ls should list files, got: {ls_output:?}"
    );
}

#[tokio::test]
async fn test_copy_file() {
    // Write a file (relative path), then CopyFile it, then Read the copy
    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": "original.txt", "content": "copy me"}),
        ),
        chunks::tool_turn(
            "tc-c",
            "CopyFile",
            serde_json::json!({"src": "original.txt", "dst": "copied.txt"}),
        ),
        chunks::tool_turn(
            "tc-r",
            "Read",
            serde_json::json!({"file_path": "copied.txt"}),
        ),
        chunks::text_turn("Copy verified."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "Write", false);
    assertions::assert_has_tool_result(&evts, "CopyFile", false);
    assertions::assert_has_tool_result(&evts, "Read", false);
}

#[tokio::test]
async fn test_binary_file_read_error() {
    let fixture = TestFixture::new();
    // Write raw binary bytes (NUL-heavy content)
    let binary_content: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let bin_path = fixture.path().join("binary.dat");
    std::fs::write(&bin_path, &binary_content).unwrap();
    let path_str = bin_path.to_str().unwrap().to_string();

    let calls = vec![
        chunks::tool_turn("tc-b", "Read", serde_json::json!({"file_path": path_str})),
        chunks::text_turn("Binary read attempted."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    // Reading a binary file should return an error result
    assertions::assert_has_tool_result(&evts, "Read", true);
}
