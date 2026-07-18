use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

/// Sandbox policy switched at runtime changes what the very next tool call
/// may do: a cwd write succeeds under `default_write`, then after
/// `SandboxPolicySwitch("read_only")` the same kind of write is blocked and
/// the file never appears.
#[tokio::test]
async fn read_only_sandbox_blocks_writes_after_runtime_switch() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "sandbox_switch",
        "calls": [
            {"expect": {"userContains": "write before"},
             "chunks": [
                {"type": "tool_use", "id": "sb1", "name": "Bash",
                 "input": {"command": "touch before-switch.txt"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "sb1"},
             "chunks": [{"type": "text", "text": "before done"}, {"type": "done"}]},
            {"expect": {"userContains": "write after"},
             "chunks": [
                {"type": "tool_use", "id": "sb2", "name": "Bash",
                 "input": {"command": "touch after-switch.txt"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "sb2"},
             "chunks": [{"type": "text", "text": "after done"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.begin_persistent().await;

    let out1 = h.turn_via_message("please write before").await;
    assert!(
        out1.finished && out1.text.contains("before done"),
        "turn 1: {out1:?}"
    );
    assert!(
        h.cwd().join("before-switch.txt").exists(),
        "default_write must allow cwd writes"
    );

    h.control_fire(json!({"SandboxPolicySwitch": "read_only"}))
        .await;
    assert!(
        h.await_event("SandboxPolicyChanged", Duration::from_secs(3))
            .await,
        "the switch must emit SandboxPolicyChanged"
    );

    let out2 = h.turn_via_message("now write after").await;
    assert!(out2.finished, "turn 2 must settle: {out2:?}");
    assert!(
        !h.cwd().join("after-switch.txt").exists(),
        "read_only must block the write; events: {:?}",
        out2.events
    );
    assert!(
        out2.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("is_error: true")),
        "the blocked write must surface as an error tool result; events: {:?}",
        out2.events
    );
}
