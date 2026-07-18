use serde_json::json;

use crate::support::CliHarness;

/// The plan-approval loop over the wire, in two rounds. Round one: a
/// premature ExitPlanMode is blocked and the error names the runtime-chosen
/// plan path. The test then writes the plan file itself (the runtime picks a
/// random slug, so a static scenario cannot Write it — the contract under
/// test is the approval round-trip, not the model's file write). Round two:
/// ExitPlanMode finds the plan, `agent/plan_approval` reaches the user seat,
/// approval restores Act mode and hands the plan back to the model.
#[tokio::test]
async fn plan_approval_round_trips_and_restores_act_mode() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "plan_mode",
        "calls": [
            {"expect": {"userContains": "plan the feature"},
             "chunks": [
                {"type": "tool_use", "id": "p1", "name": "ExitPlanMode", "input": {}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "p1"},
             "chunks": [{"type": "text", "text": "need to write the plan first"},
                        {"type": "done"}]},
            {"expect": {"userContains": "try the exit again"},
             "chunks": [
                {"type": "tool_use", "id": "p2", "name": "ExitPlanMode", "input": {}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "p2"},
             "chunks": [{"type": "text", "text": "plan approved, implementing"},
                        {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.begin_persistent_with(json!({"mode": "plan"})).await;

    let out1 = h.turn_via_message("please plan the feature").await;
    assert!(
        out1.finished && out1.text.contains("need to write the plan first"),
        "round 1: {out1:?}"
    );
    let blocked = out1
        .events
        .iter()
        .find(|e| e.starts_with("ToolResult") && e.contains("No plan file at "))
        .expect("premature ExitPlanMode must be blocked with the plan path");
    let plan_path = blocked
        .split("No plan file at ")
        .nth(1)
        .and_then(|rest| rest.split(". Write your plan").next())
        .expect("the block message names the plan path")
        .to_string();

    std::fs::create_dir_all(std::path::Path::new(&plan_path).parent().unwrap()).unwrap();
    std::fs::write(&plan_path, "# Plan\n1. paint the shed cerulean\n").unwrap();

    let out2 = h.turn_via_message("now try the exit again").await;
    assert!(
        out2.finished && out2.text.contains("plan approved, implementing"),
        "round 2: {out2:?}"
    );
    assert!(
        out2.events.iter().any(|e| e.starts_with("ToolResult")
            && e.contains("User approved your plan")
            && e.contains("paint the shed cerulean")),
        "approval must hand the plan back to the model; events: {:?}",
        out2.events
    );
    assert!(
        out2.events
            .iter()
            .any(|e| e.starts_with("ModeChanged") && e.contains("act")),
        "approval must restore Act mode; events: {:?}",
        out2.events
    );

    let requests = h.permissions().plan_requests();
    assert_eq!(
        requests.len(),
        1,
        "exactly one plan approval must reach the user seat; got {requests:?}"
    );
    assert!(
        requests[0]["plan_content"]
            .as_str()
            .is_some_and(|c| c.contains("paint the shed cerulean")),
        "the approval request must carry the plan content; request: {}",
        requests[0]
    );
}
