use std::time::Duration;

use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::Value;

use crate::HubUplink;

#[cfg(not(test))]
const ROUTE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(not(test))]
const SPAWN_TIMEOUT: Duration = Duration::from_secs(35);
#[cfg(not(test))]
const RELAY_TIMEOUT: Duration = Duration::from_secs(35);
#[cfg(test)]
const ROUTE_TIMEOUT: Duration = Duration::from_millis(100);
#[cfg(test)]
const SPAWN_TIMEOUT: Duration = Duration::from_millis(100);
#[cfg(test)]
const RELAY_TIMEOUT: Duration = Duration::from_millis(100);

impl HubUplink {
    pub async fn route(&self, envelope: &Envelope) -> Result<(), String> {
        let mut forwarded = envelope.clone();
        forwarded.apply_snat(self.hub_name());
        let params = serde_json::to_value(&forwarded)
            .map_err(|error| format!("serialize envelope: {error}"))?;
        let response = request(self, methods::META_ROUTE.name, params, ROUTE_TIMEOUT).await?;
        reject_rpc_error("meta/route", &response)?;
        Ok(())
    }

    pub async fn spawn_agent(&self, params: Value) -> Result<Value, String> {
        let response = request(self, methods::META_SPAWN.name, params, SPAWN_TIMEOUT).await?;
        reject_rpc_error("meta/spawn", &response)?;
        Ok(response)
    }

    pub async fn relay_remote(&self, params: Value) -> Result<Value, String> {
        request(self, methods::META_REMOTE_RELAY.name, params, RELAY_TIMEOUT).await
    }
}

async fn request(
    uplink: &HubUplink,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    tokio::time::timeout(timeout, uplink.connection().send_request(method, params))
        .await
        .map_err(|_| format!("{method} timed out"))?
        .map_err(|error| format!("{method} failed: {error}"))
}

fn reject_rpc_error(method: &str, response: &Value) -> Result<(), String> {
    match response.get("message").and_then(Value::as_str) {
        Some(message) => Err(format!("{method} error: {message}")),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "uplink_requests_tests.rs"]
mod tests;
