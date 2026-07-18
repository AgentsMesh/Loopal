use serde_json::json;

use crate::support::CliHarness;

/// Configured lifecycle hooks fire around a real tool execution: a
/// `pre_tool_use` hook (condition-gated to Bash) and a `post_tool_use` hook
/// each leave a marker file, proving the settings-driven hook pipeline runs
/// on both sides of the tool without disturbing the turn.
#[tokio::test]
async fn tool_hooks_fire_around_execution() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "hooks",
        "calls": [
            {"expect": {"userContains": "run the hooked tool"},
             "chunks": [
                {"type": "tool_use", "id": "h1", "name": "Bash",
                 "input": {"command": "echo hooked-tool-ran"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "h1"},
             "chunks": [{"type": "text", "text": "hooks done"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let pre_marker = h.cwd().join("pre-hook-ran.marker");
    let post_marker = h.cwd().join("post-hook-ran.marker");
    let dir = h.cwd().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec_pretty(&json!({
            "hooks": [
                {"event": "pre_tool_use", "if": "Bash(*)",
                 "command": format!("touch {}", pre_marker.display())},
                {"event": "post_tool_use",
                 "command": format!("touch {}", post_marker.display())}
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let out = h.run_turn("please run the hooked tool").await;
    assert!(
        out.finished && out.text.contains("hooks done"),
        "turn failed: {out:?}"
    );
    assert!(
        out.events.iter().any(|e| e.starts_with("ToolResult")
            && e.contains("hooked-tool-ran")
            && e.contains("is_error: false")),
        "the hooked tool itself must run normally; events: {:?}",
        out.events
    );
    assert!(
        pre_marker.exists(),
        "the pre_tool_use hook must have run before the tool"
    );
    assert!(
        post_marker.exists(),
        "the post_tool_use hook must have run after the tool"
    );
}
