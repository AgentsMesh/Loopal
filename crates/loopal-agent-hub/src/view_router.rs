//! UI ↔ Hub `view/snapshot` handler.
//!
//! UI clients seed their local replica from `view/snapshot` and then
//! follow the existing `agent/event` notification broadcast — the Hub
//! does not push separate `view/delta` notifications.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;

use loopal_view_state::ViewSnapshotRequest;

use crate::hub::Hub;

/// Handle `view/snapshot`. Returns the JSON-serialized `ViewSnapshot`
/// (or an error if the agent is not registered).
pub async fn handle_snapshot(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let req: ViewSnapshotRequest =
        serde_json::from_value(params).map_err(|e| format!("malformed view/snapshot: {e}"))?;
    let view = {
        let h = hub.lock().await;
        h.registry
            .agent_view(&req.agent)
            .ok_or_else(|| format!("no agent: '{}'", req.agent))?
    };
    let snapshot = view.lock().await.snapshot();
    serde_json::to_value(&snapshot).map_err(|e| format!("serialize snapshot: {e}"))
}
