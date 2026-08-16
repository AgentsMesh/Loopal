use super::build_replay_events;
use serde_json::json;

fn methods(events: &[(String, serde_json::Value)]) -> Vec<&str> {
    events.iter().map(|(method, _)| method.as_str()).collect()
}

#[test]
fn replays_bg_crons_tasks_mcp() {
    let state = json!({
        "bg_tasks": {
            "bg1": {"id":"bg1","description":"npm","status":"Completed","exit_code":0,"output":"done","created_at_unix_ms":1}
        },
        "crons": [{"id":"c1"}], "tasks": [{"id":"t1"}], "mcp_status": [{"name":"fs"}]
    });
    let events = build_replay_events("s", &state);
    let methods = methods(&events);
    assert!(methods.contains(&"_loopal/bgTask.spawned"));
    assert!(methods.contains(&"_loopal/bgTask.completed"));
    assert!(methods.contains(&"_loopal/crons"));
    assert!(methods.contains(&"_loopal/tasks"));
    assert!(methods.contains(&"_loopal/mcp"));
}

#[test]
fn running_bg_task_has_no_completed_event() {
    let state = json!({
        "bg_tasks": {"bg1": {"id":"bg1","description":"x","status":"Running","output":"","created_at_unix_ms":1}},
        "crons": [], "tasks": []
    });
    let events = build_replay_events("s", &state);
    let methods = methods(&events);
    assert!(methods.contains(&"_loopal/bgTask.spawned"));
    assert!(!methods.contains(&"_loopal/bgTask.completed"));
    assert!(!methods.contains(&"_loopal/bgTask.output"));
}

#[test]
fn omits_mcp_when_absent() {
    let state = json!({"bg_tasks": {}, "crons": [], "tasks": []});
    let events = build_replay_events("s", &state);
    let methods = methods(&events);
    assert!(!methods.contains(&"_loopal/mcp"));
    assert!(methods.contains(&"_loopal/crons"));
}

#[test]
fn replays_config_observables_skipping_empty() {
    let state = json!({
        "agent": {"observable": {
            "permission_mode": "bypass", "model": "opus", "mode": "", "thinking_config": "auto"
        }},
        "bg_tasks": {}, "crons": [], "tasks": []
    });
    let events = build_replay_events("s", &state);
    let methods = methods(&events);
    assert!(methods.contains(&"_loopal/permission_mode"));
    assert!(methods.contains(&"_loopal/model"));
    assert!(!methods.contains(&"_loopal/mode"));
    assert!(methods.contains(&"_loopal/thinking"));
    let by = |key: &str| {
        events
            .iter()
            .find(|(method, _)| method == key)
            .unwrap()
            .1
            .clone()
    };
    assert_eq!(
        by("_loopal/permission_mode")["data"]["permission_mode"],
        "bypass"
    );
    assert_eq!(by("_loopal/thinking")["data"]["thinking"], "auto");
}

#[test]
fn replays_active_and_recent_workflows_through_typed_notification() {
    let workflow = |id: &str, state: &str| {
        json!({
            "id": id, "run_goal": format!("goal-{id}"), "state": state, "revision": 3,
            "output_node": "output",
            "counts": {"pending":0,"ready":0,"active":1,"succeeded":0,"failed":0,"cancelled":0,"skipped":0},
            "created_at_unix_ms": 10, "updated_at_unix_ms": 20
        })
    };
    let active = workflow("wrun-active", "running");
    let recent = workflow("wrun-recent", "succeeded");
    let state = json!({
        "bg_tasks": {}, "crons": [], "tasks": [],
        "workflows": {"active": [active.clone()], "recent": [recent.clone()]}
    });
    let workflows: Vec<_> = build_replay_events("s", &state)
        .into_iter()
        .filter(|(method, _)| method == "_loopal/workflow")
        .collect();
    assert_eq!(workflows.len(), 2);
    assert_eq!(workflows[0].1["sessionId"], "s");
    assert_eq!(workflows[0].1["data"]["workflow"], active);
    assert_eq!(workflows[1].1["data"]["workflow"], recent);
}
