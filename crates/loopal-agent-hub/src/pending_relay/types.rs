use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::AgentEventPayload;

use crate::HubUplink;

#[derive(Clone)]
pub enum InteractionAudience {
    LocalUi,
    RemoteUi {
        target_hub: String,
        uplink: Arc<HubUplink>,
    },
}

impl InteractionAudience {
    pub fn is_local(&self) -> bool {
        matches!(self, Self::LocalUi)
    }
}

pub struct PendingPermissionInfo {
    pub agent_conn: Arc<Connection<Listening>>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
    /// Opaque Hub-issued capability carried by Request/Response/Resolved.
    pub interaction_id: String,
    /// Agent-owned id. Never accepted as authority from a UI response.
    pub logical_id: String,
    pub tool_name: String,
}

pub struct PendingQuestionInfo {
    pub agent_conn: Arc<Connection<Listening>>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
    /// Opaque Hub-issued capability carried by Request/Response/Resolved.
    pub interaction_id: String,
    /// Agent-owned id used only when replying to the agent/runtime.
    pub logical_id: String,
    pub audience: InteractionAudience,
}

pub struct PendingPlanApprovalInfo {
    pub agent_conn: Arc<Connection<Listening>>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
    /// Opaque Hub-issued capability carried by Request/Response/Resolved.
    pub interaction_id: String,
    /// Agent-owned id used for duplicate admission, never UI authority.
    pub logical_id: String,
}

#[derive(Clone)]
pub struct PendingRemoteQuestionInfo {
    pub origin_hub: String,
    pub origin_agent: String,
    pub qualified_agent: String,
    pub interaction_id: String,
    pub logical_id: String,
    pub request: AgentEventPayload,
    pub uplink: Arc<HubUplink>,
    pub deadline: tokio::time::Instant,
    pub forwarding: bool,
}

#[derive(Clone)]
pub(crate) struct OriginRemoteQuestionCancel {
    pub target_hub: String,
    pub origin_agent: String,
    pub interaction_id: String,
    pub uplink: Arc<HubUplink>,
}

pub(super) enum FastPath {
    DenyNoUi,
    RejectDuplicate,
    EmitFailed,
    Pending,
}
