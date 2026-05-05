use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;

pub async fn handle_hub_shutdown(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    let signal = hub.lock().await.shutdown_signal.clone();
    // notify_one stores a permit if no waiter exists yet — protects against
    // the race where TUI sends hub/shutdown before hub_only registers
    // notified().await.
    signal.notify_one();
    Ok(json!({ "ok": true }))
}
