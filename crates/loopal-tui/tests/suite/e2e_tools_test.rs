//! E2E tests for tool execution: parallel tools, write+read roundtrip, bash, grep.

use loopal_test_support::{TestFixture, assertions, chunks, events};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_parallel_tool_execution() {
    // Use fixture tempdir so Ls works inside sandbox
    let fixture = TestFixture::new();
    fixture.create_file("a.txt", "a");
    fixture.create_file("b.txt", "b");
    let dir = fixture.path().to_str().unwrap().to_string();

    let calls = vec![
        vec![
            chunks::tool_use("tc-1", "Ls", serde_json::json!({"path": &dir})),
            chunks::tool_use("tc-2", "Ls", serde_json::json!({"path": &dir})),
            chunks::tool_use("tc-3", "Ls", serde_json::json!({"path": &dir})),
            chunks::usage(15, 10),
            chunks::done(),
        ],
        chunks::text_turn("All three tools executed."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    let tool_names = events::extract_tool_names(&evts);
    assert_eq!(
        tool_names.len(),
        3,
        "expected 3 ToolCall events, got: {tool_names:?}"
    );

    let results = events::extract_tool_results(&evts);
    assert_eq!(
        results.len(),
        3,
        "expected 3 ToolResult events, got: {results:?}"
    );
}

#[tokio::test]
async fn test_write_then_read_roundtrip() {
    let fixture = TestFixture::new();
    let file_path = fixture.path().join("roundtrip.txt");
    let path_str = file_path.to_str().unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": path_str, "content": "hello roundtrip"}),
        ),
        chunks::tool_turn("tc-r", "Read", serde_json::json!({"file_path": path_str})),
        chunks::text_turn("Roundtrip complete."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    // Write should succeed
    assertions::assert_has_tool_result(&evts, "Write", false);
    // Read should succeed
    assertions::assert_has_tool_result(&evts, "Read", false);
    assertions::assert_has_stream(&evts);
}

#[tokio::test]
async fn test_bash_echo() {
    let calls = vec![
        chunks::tool_turn(
            "tc-b",
            "Bash",
            serde_json::json!({"command": "echo hello_from_bash"}),
        ),
        chunks::text_turn("Bash done."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "Bash");
    assertions::assert_has_tool_result(&evts, "Bash", false);

    // Verify output contains the echo string
    let results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            loopal_protocol::AgentEventPayload::ToolResult { name, result, .. }
                if name == "Bash" =>
            {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        results.iter().any(|r| r.contains("hello_from_bash")),
        "bash output should contain 'hello_from_bash', got: {results:?}"
    );
}

#[tokio::test]
async fn test_grep_search() {
    let fixture = TestFixture::new();
    let file_path = fixture.create_file("searchme.txt", "line1\ntarget_line_here\nline3\n");
    let dir_path = fixture.path().to_str().unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-g",
            "Grep",
            serde_json::json!({"pattern": "target_line", "path": dir_path}),
        ),
        chunks::text_turn("Grep done."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "Grep");
    assertions::assert_has_tool_result(&evts, "Grep", false);

    // Keep fixture alive until events collected
    let _ = file_path;
}

#[tokio::test]
async fn test_tool_results_include_names() {
    let fixture = TestFixture::new();
    let dir = fixture.path().to_str().unwrap().to_string();
    fixture.create_file("dummy.txt", "hello");
    let calls = vec![
        chunks::tool_turn("tc-1", "Ls", serde_json::json!({"path": dir})),
        chunks::text_turn("Done."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    let results = events::extract_tool_results(&evts);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "Ls");
    assert!(!results[0].1, "Ls should not be error");
}
