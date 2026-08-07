use std::time::Duration;

use loopal_ipc::cross_hub::RemoteSpawnOutcome;
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::Value;

use crate::HubUplink;

#[derive(Debug)]
pub(crate) enum SpawnAgentRequestError {
    /// The remote endpoint returned an application rejection. By protocol,
    /// this response means no child remains running and the local shadow may
    /// be rolled back.
    Rejected(String),
    /// Timeout/transport loss after request admission. The remote side may
    /// have spawned the child, so its shadow must remain for late completion.
    OutcomeUnknown(String),
}

impl std::fmt::Display for SpawnAgentRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(message) | Self::OutcomeUnknown(message) => formatter.write_str(message),
        }
    }
}

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
        self.spawn_agent_classified(params)
            .await
            .map_err(|error| error.to_string())
    }

    pub(crate) async fn spawn_agent_classified(
        &self,
        params: Value,
    ) -> Result<Value, SpawnAgentRequestError> {
        let response = tokio::time::timeout(
            SPAWN_TIMEOUT,
            self.connection()
                .send_request(methods::META_SPAWN.name, params),
        )
        .await
        .map_err(|_| SpawnAgentRequestError::OutcomeUnknown("meta/spawn timed out".into()))?
        .map_err(|error| {
            // A JSON-RPC transport/remote error does not prove whether an
            // older or failed MetaHub forwarded the request before replying.
            SpawnAgentRequestError::OutcomeUnknown(format!("meta/spawn failed: {error}"))
        })?;

        match serde_json::from_value::<RemoteSpawnOutcome>(response.clone()) {
            Ok(RemoteSpawnOutcome::Spawned { response }) => Ok(response),
            Ok(RemoteSpawnOutcome::RejectedBeforeSideEffect { message }) => {
                Err(SpawnAgentRequestError::Rejected(message))
            }
            Ok(RemoteSpawnOutcome::OutcomeUnknown { message }) => {
                Err(SpawnAgentRequestError::OutcomeUnknown(message))
            }
            // Backward compatibility for direct test peers and older MetaHubs.
            // Successful legacy values are definitive only when they contain a
            // spawn id. A legacy `{message}` result was the old rejection wire
            // form; anything else is an unknown protocol outcome.
            Err(_) if response.get("agent_id").and_then(Value::as_str).is_some() => Ok(response),
            Err(_) if response.get("message").and_then(Value::as_str).is_some() => {
                Err(SpawnAgentRequestError::Rejected(format!(
                    "meta/spawn error: {}",
                    response["message"].as_str().unwrap_or("remote rejection")
                )))
            }
            Err(error) => Err(SpawnAgentRequestError::OutcomeUnknown(format!(
                "invalid meta/spawn outcome: {error}"
            ))),
        }
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
