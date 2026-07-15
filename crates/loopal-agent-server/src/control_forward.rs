use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::jsonrpc;
use loopal_protocol::ControlCommand;
use loopal_runtime::agent_input::{AgentInput, ControlAcknowledgement, ControlRequest};

use crate::session_hub::SharedSession;

const CONTROL_APPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

pub(crate) fn spawn(
    request_id: i64,
    params: serde_json::Value,
    session: Arc<SharedSession>,
    connection: Arc<Connection<Listening>>,
) {
    tokio::spawn(async move {
        forward(request_id, params, &session, &connection).await;
    });
}

async fn forward(
    request_id: i64,
    params: serde_json::Value,
    session: &SharedSession,
    connection: &Connection<Listening>,
) {
    let command = match serde_json::from_value::<ControlCommand>(params) {
        Ok(command) => command,
        Err(error) => {
            reject(
                connection,
                request_id,
                format!("invalid control command: {error}"),
            )
            .await;
            return;
        }
    };
    let (request, mut acknowledgement) = ControlRequest::tracked(command);
    if session
        .input_tx
        .send(AgentInput::TrackedControl(request))
        .await
        .is_err()
    {
        reject(connection, request_id, "agent input channel is closed").await;
        return;
    }
    match wait_for_acknowledgement(&mut acknowledgement, CONTROL_APPLY_TIMEOUT).await {
        AckWait::Received(ControlAcknowledgement::Applied) => {
            let _ = connection
                .respond(request_id, serde_json::json!({"status": "applied"}))
                .await;
        }
        AckWait::Received(ControlAcknowledgement::Rejected(reason)) => {
            reject(
                connection,
                request_id,
                format!("control rejected: {reason}"),
            )
            .await;
        }
        AckWait::Closed => {
            reject(
                connection,
                request_id,
                "control application channel closed before acknowledgement",
            )
            .await;
        }
        AckWait::TimedOut => {
            acknowledgement.close();
            reject(connection, request_id, "control application timed out").await;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum AckWait {
    Received(ControlAcknowledgement),
    Closed,
    TimedOut,
}

async fn wait_for_acknowledgement(
    receiver: &mut tokio::sync::mpsc::Receiver<ControlAcknowledgement>,
    timeout: std::time::Duration,
) -> AckWait {
    match tokio::time::timeout(timeout, receiver.recv()).await {
        Ok(Some(acknowledgement)) => AckWait::Received(acknowledgement),
        Ok(None) => AckWait::Closed,
        Err(_) => AckWait::TimedOut,
    }
}

async fn reject(connection: &Connection<Listening>, request_id: i64, reason: impl AsRef<str>) {
    let _ = connection
        .respond_error(request_id, jsonrpc::INVALID_REQUEST, reason.as_ref())
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acknowledgement_wait_is_bounded() {
        let (_sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let result =
            wait_for_acknowledgement(&mut receiver, std::time::Duration::from_millis(1)).await;
        assert_eq!(result, AckWait::TimedOut);
    }

    #[tokio::test]
    async fn dropped_acknowledgement_finishes_without_waiting() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        drop(sender);
        let result =
            wait_for_acknowledgement(&mut receiver, std::time::Duration::from_secs(1)).await;
        assert_eq!(result, AckWait::Closed);
    }
}
