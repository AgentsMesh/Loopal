use serde_json::json;

use crate::support::CliHarness;

fn dangerous_tool_scenario(name: &str, final_text: &str) -> serde_json::Value {
    json!({
        "version": 2,
        "name": name,
        "calls": [
            {"expect": {"userContains": "run the command"},
             "chunks": [
                {"type": "tool_use", "id": "p1", "name": "Bash",
                 "input": {"command": "echo permission-gated-ran"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "p1"},
             "chunks": [{"type": "text", "text": final_text}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    })
}

/// Under `ask_dangerous` + manual decisions, a Dangerous tool must ask the
/// user seat over IPC (`agent/permission`) before executing; on allow, the
/// tool runs and the turn completes normally.
#[tokio::test]
async fn dangerous_tool_asks_and_runs_when_allowed() {
    let mut h = CliHarness::start(dangerous_tool_scenario("perm_allow", "allowed done")).await;
    h.permissions().set_allow(true);

    let out = h
        .run_turn_with(
            "run the command",
            json!({"permission_mode": "ask_dangerous"}),
        )
        .await;
    assert!(
        out.finished && out.text.contains("allowed done"),
        "turn failed: {out:?}"
    );
    assert!(
        out.events.iter().any(|e| e.starts_with("ToolResult")
            && e.contains("permission-gated-ran")
            && e.contains("is_error: false")),
        "allowed tool must actually execute; events: {:?}",
        out.events
    );

    let asks = h.permissions().asks();
    assert_eq!(asks.len(), 1, "exactly one permission ask; got {asks:?}");
    assert_eq!(asks[0]["tool_name"], "Bash");
    assert_eq!(
        asks[0]["tool_input"]["command"],
        "echo permission-gated-ran"
    );
}

/// Denial path: the ask comes back `allow: false`, the tool must NOT run, the
/// model receives the denial as an error tool result and still finishes the
/// turn gracefully.
#[tokio::test]
async fn dangerous_tool_denied_skips_execution_and_turn_continues() {
    let mut h = CliHarness::start(dangerous_tool_scenario("perm_deny", "denial handled")).await;
    h.permissions().set_allow(false);

    let out = h
        .run_turn_with(
            "run the command",
            json!({"permission_mode": "ask_dangerous"}),
        )
        .await;
    assert!(
        out.finished && out.text.contains("denial handled"),
        "the model must receive the denial and continue; out: {out:?}"
    );
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("is_error: true")),
        "denial must surface as an error tool result; events: {:?}",
        out.events
    );
    // The ToolCall event echoes the model's requested input, so only a
    // ToolResult carrying the command's output would prove execution.
    assert!(
        !out.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("permission-gated-ran")),
        "a denied tool must never execute; events: {:?}",
        out.events
    );
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("PermissionDecided") && e.contains("deny")),
        "the denial decision must be surfaced as an event; events: {:?}",
        out.events
    );
    assert_eq!(h.permissions().asks().len(), 1);
}

/// Classifier decision mode: the permission question goes to a classifier LLM
/// call over the same wire (its user prompt embeds the tool input); a
/// `should_block: false` verdict approves the tool with the user seat silent.
#[tokio::test]
async fn classifier_decides_permission_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "perm_classifier",
        "calls": [
            {"expect": {"userContains": "run the command"},
             "chunks": [
                {"type": "tool_use", "id": "p1", "name": "Bash",
                 "input": {"command": "echo classifier-approved-ran"}},
                {"type": "done"}
             ]},
            {"expect": {"userContains": "classifier-approved-ran"},
             "chunks": [
                {"type": "text",
                 "text": "{\"should_block\": false, \"reason\": \"normal dev command\"}"},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "p1"},
             "chunks": [{"type": "text", "text": "classifier allowed done"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.permissions().set_hold(true);

    let out = h
        .run_turn_with(
            "run the command",
            json!({"permission_mode": "ask_dangerous", "decision_mode": "classifier"}),
        )
        .await;
    assert!(
        out.finished && out.text.contains("classifier allowed done"),
        "turn failed: {out:?}"
    );
    assert!(
        out.events.iter().any(|e| e.starts_with("ToolResult")
            && e.contains("classifier-approved-ran")
            && e.contains("is_error: false")),
        "the classifier verdict must approve and run the tool; events: {:?}",
        out.events
    );
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("ClassifierCompleted") || e.contains("classifier")),
        "the classifier decision must be observable in events; events: {:?}",
        out.events
    );
}

/// Bypass mode (the suite default) must never ask: the same Dangerous tool
/// executes without any `agent/permission` round-trip.
#[tokio::test]
async fn bypass_mode_never_asks() {
    let mut h = CliHarness::start(dangerous_tool_scenario("perm_bypass", "bypass done")).await;

    let out = h.run_turn("run the command").await;
    assert!(
        out.finished && out.text.contains("bypass done"),
        "turn failed: {out:?}"
    );
    assert!(
        h.permissions().asks().is_empty(),
        "bypass must not ask; asks: {:?}",
        h.permissions().asks()
    );
}
