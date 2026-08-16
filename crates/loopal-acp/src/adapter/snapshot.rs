//! Cold-start replay: on `session/new`, fetch the agent's `view/snapshot` and
//! replay it as `_loopal/*` incremental events so the control panel rebuilds
//! prior state (resumed sessions, pre-existing cron / bg-tasks). Reusing the
//! incremental path means no snapshot message type is needed downstream — the
//! runner accumulator and core reducer absorb it through their normal handlers.

use serde_json::{Value, json};

use loopal_ipc::protocol::methods::VIEW_SNAPSHOT;
use loopal_protocol::{ROOT_AGENT_NAME, WorkflowRunsSnapshot};

#[cfg(not(test))]
const SNAPSHOT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(test)]
const SNAPSHOT_DEADLINE: std::time::Duration = std::time::Duration::from_millis(200);

use super::AcpAdapter;
use crate::translate::ext::{ext_notification, workflow_run_changed};

impl AcpAdapter {
    pub(crate) async fn replay_loopal_snapshot(&self, session_id: &str) {
        let Some(state) = self.fetch_root_view_state().await else {
            return;
        };
        for (method, params) in build_replay_events(session_id, &state) {
            self.acp_out.notify(&method, params).await;
        }
    }

    async fn fetch_root_view_state(&self) -> Option<Value> {
        let request = self
            .client
            .connection()
            .send_request(VIEW_SNAPSHOT.name, json!({ "agent": ROOT_AGENT_NAME }));
        let resp = tokio::time::timeout(SNAPSHOT_DEADLINE, request)
            .await
            .ok()?
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
    push_workflows(session_id, state, &mut events);
    push_config_observables(session_id, state, &mut events);
    events
}

fn push_workflows(session_id: &str, state: &Value, events: &mut Vec<(String, Value)>) {
    let Ok(workflows) = serde_json::from_value::<WorkflowRunsSnapshot>(state["workflows"].clone())
    else {
        return;
    };
    for workflow in workflows.active.iter().chain(&workflows.recent) {
        events.push(workflow_run_changed(session_id, workflow));
    }
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
#[path = "snapshot_tests.rs"]
mod tests;
