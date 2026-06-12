//! Cold-start replay: on `session/new`, fetch the agent's `view/snapshot` and
//! replay it as `_loopal/*` incremental events so the control panel rebuilds
//! prior state (resumed sessions, pre-existing cron / bg-tasks). Reusing the
//! incremental path means no snapshot message type is needed downstream — the
//! runner accumulator and core reducer absorb it through their normal handlers.

use serde_json::{Value, json};

use loopal_ipc::protocol::methods::VIEW_SNAPSHOT;

use super::AcpAdapter;
use crate::translate::ext::ext_notification;

impl AcpAdapter {
    pub(crate) async fn replay_loopal_snapshot(&self, session_id: &str) {
        let Some(agent) = self.first_agent_name().await else {
            return;
        };
        let Some(state) = self.fetch_view_state(&agent).await else {
            return;
        };
        for (method, params) in build_replay_events(session_id, &state) {
            self.acp_out.notify(&method, params).await;
        }
    }

    async fn first_agent_name(&self) -> Option<String> {
        let resp = self.client.list_agents().await.ok()?;
        resp.get("agents")?
            .as_array()?
            .first()?
            .get("name")?
            .as_str()
            .map(String::from)
    }

    async fn fetch_view_state(&self, agent: &str) -> Option<Value> {
        let resp = self
            .client
            .connection()
            .send_request(VIEW_SNAPSHOT.name, json!({ "agent": agent }))
            .await
            .ok()?;
        resp.get("state").cloned()
    }
}

/// Serialize a `view/snapshot` `state` into the `_loopal/*` event sequence that
/// rebuilds the control panel. Pure (no IO) so the fold rules are unit-testable;
/// `replay_loopal_snapshot` only adds the fetch + notify around it.
fn build_replay_events(session_id: &str, state: &Value) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    if let Some(map) = state["bg_tasks"].as_object() {
        for bg in map.values() {
            events.push(ext_notification(
                session_id,
                "bgTask.spawned",
                json!({
                    "id": bg["id"],
                    "description": bg["description"],
                    "created_at_unix_ms": bg["created_at_unix_ms"],
                }),
            ));
            if bg["output"].as_str().is_some_and(|s| !s.is_empty()) {
                events.push(ext_notification(
                    session_id,
                    "bgTask.output",
                    json!({ "id": bg["id"], "output_delta": bg["output"] }),
                ));
            }
            if bg["status"].as_str().is_some_and(|s| s != "Running") {
                events.push(ext_notification(
                    session_id,
                    "bgTask.completed",
                    json!({
                        "id": bg["id"],
                        "status": bg["status"],
                        "exit_code": bg["exit_code"],
                        "output": bg["output"],
                    }),
                ));
            }
        }
    }
    events.push(ext_notification(
        session_id,
        "crons",
        json!({ "crons": state["crons"] }),
    ));
    events.push(ext_notification(
        session_id,
        "tasks",
        json!({ "tasks": state["tasks"] }),
    ));
    if !state["mcp_status"].is_null() {
        events.push(ext_notification(
            session_id,
            "mcp",
            json!({ "servers": state["mcp_status"] }),
        ));
    }
    push_config_observables(session_id, state, &mut events);
    events
}

/// Replay the agent's config observables (permission_mode / model / mode /
/// thinking) from `state.agent.observable`. The bootstrap broadcast emits these
/// on agent start, but `session/new` drains that channel before the IDE
/// subscribes — the snapshot is the only cold-start source. `thinking` reads the
/// snapshot's `thinking_config` but emits under `thinking` to match the live
/// `ThinkingChanged` notification's field name.
fn push_config_observables(session_id: &str, state: &Value, events: &mut Vec<(String, Value)>) {
    let obs = &state["agent"]["observable"];
    let mut push = |ext_type: &str, raw: &Value| {
        if let Some(s) = raw.as_str().filter(|s| !s.is_empty()) {
            let data = Value::Object(
                [(ext_type.to_string(), Value::String(s.to_string()))]
                    .into_iter()
                    .collect(),
            );
            events.push(ext_notification(session_id, ext_type, data));
        }
    };
    push("permission_mode", &obs["permission_mode"]);
    push("model", &obs["model"]);
    push("mode", &obs["mode"]);
    push("thinking", &obs["thinking_config"]);
}

#[cfg(test)]
mod tests {
    use super::build_replay_events;
    use serde_json::json;

    fn methods(events: &[(String, serde_json::Value)]) -> Vec<&str> {
        events.iter().map(|(m, _)| m.as_str()).collect()
    }

    #[test]
    fn replays_bg_crons_tasks_mcp() {
        let state = json!({
            "bg_tasks": {
                "bg1": {"id":"bg1","description":"npm","status":"Completed","exit_code":0,"output":"done","created_at_unix_ms":1}
            },
            "crons": [{"id":"c1"}],
            "tasks": [{"id":"t1"}],
            "mcp_status": [{"name":"fs"}]
        });
        let ev = build_replay_events("s", &state);
        let m = methods(&ev);
        assert!(m.contains(&"_loopal/bgTask.spawned"));
        assert!(m.contains(&"_loopal/bgTask.completed")); // status != Running
        assert!(m.contains(&"_loopal/crons"));
        assert!(m.contains(&"_loopal/tasks"));
        assert!(m.contains(&"_loopal/mcp"));
    }

    #[test]
    fn running_bg_task_has_no_completed_event() {
        let state = json!({
            "bg_tasks": {"bg1": {"id":"bg1","description":"x","status":"Running","output":"","created_at_unix_ms":1}},
            "crons": [], "tasks": []
        });
        let ev = build_replay_events("s", &state);
        let m = methods(&ev);
        assert!(m.contains(&"_loopal/bgTask.spawned"));
        assert!(!m.contains(&"_loopal/bgTask.completed"));
        assert!(!m.contains(&"_loopal/bgTask.output")); // empty output
    }

    #[test]
    fn omits_mcp_when_absent() {
        let state = json!({"bg_tasks": {}, "crons": [], "tasks": []});
        let ev = build_replay_events("s", &state);
        let m = methods(&ev);
        assert!(!m.contains(&"_loopal/mcp"));
        assert!(m.contains(&"_loopal/crons"));
    }

    #[test]
    fn replays_config_observables_skipping_empty() {
        let state = json!({
            "agent": {"observable": {
                "permission_mode": "bypass", "model": "opus", "mode": "", "thinking_config": "auto"
            }},
            "bg_tasks": {}, "crons": [], "tasks": []
        });
        let ev = build_replay_events("s", &state);
        let m = methods(&ev);
        assert!(m.contains(&"_loopal/permission_mode"));
        assert!(m.contains(&"_loopal/model"));
        assert!(!m.contains(&"_loopal/mode")); // empty → skipped
        assert!(m.contains(&"_loopal/thinking"));
        let by = |k: &str| ev.iter().find(|(meth, _)| meth == k).unwrap().1.clone();
        assert_eq!(
            by("_loopal/permission_mode")["data"]["permission_mode"],
            "bypass"
        );
        assert_eq!(by("_loopal/thinking")["data"]["thinking"], "auto"); // thinking_config → thinking
    }
}
