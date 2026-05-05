use std::sync::Arc;

use loopal_ipc::connection::Connection;

pub struct PendingPermissionInfo {
    pub agent_conn: Arc<Connection>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
}

pub struct PendingQuestionInfo {
    pub agent_conn: Arc<Connection>,
    pub agent_ipc_id: i64,
    pub agent_name: String,
}

pub(super) enum FastPath {
    DenyNoUi,
    EmitFailed,
    Pending,
}
