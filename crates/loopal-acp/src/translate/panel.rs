//! `_loopal/*` extension notifications for control-panel signals (bg shell /
//! cron / task / topology / mcp / goal). Field names mirror loopal-protocol
//! snapshot types (snake_case) so the GUI state mirror needs no per-field remap.

use loopal_protocol::AgentEventPayload;

use super::AcpNotification;
use super::ext::ext_notification;

pub(crate) fn translate_panel(
    payload: &AgentEventPayload,
    session_id: &str,
) -> Option<AcpNotification> {
    let (method, params) = match payload {
        AgentEventPayload::BgTaskSpawned {
            id,
            description,
            created_at_unix_ms,
        } => ext_notification(
            session_id,
            "bgTask.spawned",
            serde_json::json!({
                "id": id,
                "description": description,
                "created_at_unix_ms": created_at_unix_ms,
            }),
        ),
        AgentEventPayload::BgTaskOutput { id, output_delta } => ext_notification(
            session_id,
            "bgTask.output",
            serde_json::json!({ "id": id, "output_delta": output_delta }),
        ),
        AgentEventPayload::BgTaskCompleted {
            id,
            status,
            exit_code,
            output,
        } => ext_notification(
            session_id,
            "bgTask.completed",
            serde_json::json!({
                "id": id,
                "status": status,
                "exit_code": exit_code,
                "output": output,
            }),
        ),
        AgentEventPayload::CronsChanged { crons } => {
            ext_notification(session_id, "crons", serde_json::json!({ "crons": crons }))
        }
        AgentEventPayload::TasksChanged { tasks } => {
            ext_notification(session_id, "tasks", serde_json::json!({ "tasks": tasks }))
        }
        AgentEventPayload::McpStatusReport { servers } => {
            ext_notification(session_id, "mcp", serde_json::json!({ "servers": servers }))
        }
        AgentEventPayload::SubAgentSpawned(spawn) => ext_notification(
            session_id,
            "topology.spawn",
            serde_json::json!({ "spawn": spawn }),
        ),
        AgentEventPayload::ThreadGoalUpdated { goal, .. } => {
            ext_notification(session_id, "goal", serde_json::json!({ "goal": goal }))
        }
        AgentEventPayload::ModelChanged { model } => {
            ext_notification(session_id, "model", serde_json::json!({ "model": model }))
        }
        // thinking_config is already-serialized ThinkingConfig JSON; pass through
        // raw so the GUI normalizes it to a label without a second schema here.
        AgentEventPayload::ThinkingChanged { thinking_config } => ext_notification(
            session_id,
            "thinking",
            serde_json::json!({ "thinking": thinking_config }),
        ),
        AgentEventPayload::PermissionModeChanged { mode } => ext_notification(
            session_id,
            "permission_mode",
            serde_json::json!({ "permission_mode": mode }),
        ),
        _ => return None,
    };
    Some(AcpNotification::Extension { method, params })
}
