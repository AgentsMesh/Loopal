use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};

pub struct PendingPermissionInfo {
    pub agent_conn: Arc<Connection<Listening>>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
    pub tool_name: String,
}

pub struct PendingQuestionInfo {
    pub agent_conn: Arc<Connection<Listening>>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
}

pub struct PendingPlanApprovalInfo {
    pub agent_conn: Arc<Connection<Listening>>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
}

pub(super) enum FastPath {
    DenyNoUi,
    EmitFailed,
    Pending,
}
