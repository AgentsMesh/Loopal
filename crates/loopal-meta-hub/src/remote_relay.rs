use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;

use loopal_ipc::protocol::methods;

use crate::MetaHub;

const RELAY_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn forward(
    meta_hub: &Arc<Mutex<MetaHub>>,
    mut params: Value,
) -> Result<Value, String> {
    let target_hub = params["target_hub"]
        .as_str()
        .ok_or("remote relay missing target_hub")?
        .to_string();
    let operation = params["operation"]
        .as_str()
        .ok_or("remote relay missing operation")?;
    if !matches!(
        operation,
        "question_request" | "question_response" | "question_cancel" | "control" | "interrupt"
    ) {
        return Err(format!("unsupported remote relay operation: {operation}"));
    }
    if let Some(object) = params.as_object_mut() {
        object.remove("target_hub");
    }
    let connection = meta_hub
        .lock()
        .await
        .registry
        .connection(&target_hub)
        .ok_or_else(|| format!("hub '{target_hub}' not connected"))?;
    tokio::time::timeout(
        RELAY_TIMEOUT,
        connection.send_request(methods::HUB_REMOTE_RELAY.name, params),
    )
    .await
    .map_err(|_| format!("remote relay to '{target_hub}' timed out"))?
    .map_err(|error| format!("remote relay to '{target_hub}' failed: {error}"))
}
