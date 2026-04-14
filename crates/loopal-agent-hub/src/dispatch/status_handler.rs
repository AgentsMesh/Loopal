//! Hub status query handler.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;

pub async fn handle_status(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    let h = hub.lock().await;
    let uplink_info = h.uplink.as_ref().map(|u| {
        json!({
            "connected": true,
            "hub_name": u.hub_name(),
        })
    });
    Ok(json!({
        "agent_count": h.registry.agent_count(),
        "uplink": uplink_info,
    }))
}
