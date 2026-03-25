//! E2E tests for Task tools (TaskCreate, TaskUpdate, TaskList, TaskGet).

use loopal_test_support::{assertions, chunks, events};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_task_create() {
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "TaskCreate",
            serde_json::json!({
                "subject": "Fix the bug",
                "description": "There's a critical bug in login"
            }),
        ),
        chunks::text_turn("Task created."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "TaskCreate");
    assertions::assert_has_tool_result(&evts, "TaskCreate", false);
    let results = events::extract_tool_results(&evts);
    assert!(
        results
            .iter()
            .any(|(name, err)| name == "TaskCreate" && !err)
    );
}

#[tokio::test]
async fn test_task_update_status() {
    // Create then update in sequence. TaskStore generates IDs starting at 1.
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "TaskCreate",
            serde_json::json!({ "subject": "Test task", "description": "desc" }),
        ),
        chunks::tool_turn(
            "tc-2",
            "TaskUpdate",
            serde_json::json!({ "taskId": "1", "status": "in_progress" }),
        ),
        chunks::text_turn("Updated."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "TaskCreate");
    assertions::assert_has_tool_call(&evts, "TaskUpdate");
    assertions::assert_has_tool_result(&evts, "TaskUpdate", false);
}

#[tokio::test]
async fn test_task_list() {
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "TaskCreate",
            serde_json::json!({ "subject": "Task A", "description": "first" }),
        ),
        chunks::tool_turn(
            "tc-2",
            "TaskCreate",
            serde_json::json!({ "subject": "Task B", "description": "second" }),
        ),
        chunks::tool_turn("tc-3", "TaskList", serde_json::json!({})),
        chunks::text_turn("Listed."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "TaskList", false);
}

#[tokio::test]
async fn test_task_get() {
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "TaskCreate",
            serde_json::json!({ "subject": "Get me", "description": "test" }),
        ),
        chunks::tool_turn("tc-2", "TaskGet", serde_json::json!({ "taskId": "1" })),
        chunks::text_turn("Got it."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "TaskGet", false);
}

#[tokio::test]
async fn test_task_not_found() {
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "TaskGet",
            serde_json::json!({ "taskId": "nonexistent" }),
        ),
        chunks::text_turn("Not found."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "TaskGet", true);
}
