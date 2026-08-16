use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    WorkflowAttemptState, WorkflowWorkerHandshakeDisposition, WorkflowWorkerHandshakeRequest,
    WorkflowWorkerHandshakeResponse,
};

use crate::params::StartParams;

const WORKER_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);

pub(super) async fn send_if_worker(
    connection: &Arc<Connection<Listening>>,
    start: &StartParams,
) -> anyhow::Result<()> {
    let causation = start.workflow_permission_causation.clone();
    let capability = start.workflow_attempt_capability.clone();
    let (causation, capability) = match (causation, capability) {
        (None, None) => return Ok(()),
        (Some(causation), Some(capability)) => (causation, capability),
        _ => anyhow::bail!(
            "workflow attempt capability and permission causation must be supplied together"
        ),
    };
    let method = methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name;
    let params = serde_json::to_value(WorkflowWorkerHandshakeRequest {
        causation,
        capability,
    })?;
    let response = tokio::time::timeout(
        WORKER_HANDSHAKE_TIMEOUT,
        connection.send_request(method, params),
    )
    .await
    .map_err(|_| anyhow::anyhow!("{method} timed out"))?
    .map_err(|error| anyhow::anyhow!("{method} rejected worker startup: {error}"))?;
    let response: WorkflowWorkerHandshakeResponse = serde_json::from_value(response)
        .map_err(|error| anyhow::anyhow!("{method} returned an invalid response: {error}"))?;
    validate_response(response).map_err(anyhow::Error::msg)
}

fn validate_response(response: WorkflowWorkerHandshakeResponse) -> Result<(), String> {
    match (response.disposition, response.attempt_state) {
        (
            WorkflowWorkerHandshakeDisposition::Fresh,
            WorkflowAttemptState::Dispatching | WorkflowAttemptState::Running,
        )
        | (WorkflowWorkerHandshakeDisposition::Recovered, WorkflowAttemptState::Running) => Ok(()),
        (disposition, state) => Err(format!(
            "{} acknowledged an invalid startup state: {disposition:?}/{state:?}",
            methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name
        )),
    }
}

#[cfg(test)]
#[path = "workflow_handshake_tests.rs"]
mod tests;
