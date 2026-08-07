use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::jsonrpc;
use loopal_protocol::{ControlCommand, ControlDisposition};
use loopal_runtime::agent_input::{AgentInput, ControlAcknowledgement, ControlRequest};

use crate::session_hub::SharedSession;

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
    match wait_for_acknowledgement(
        &mut acknowledgement,
        loopal_protocol::DEFAULT_CONTROL_APPLICATION_TIMEOUT,
    )
    .await
    {
        AckWait::Received(ControlAcknowledgement::Applied) => {
            respond_disposition(connection, request_id, ControlDisposition::Applied).await;
        }
        AckWait::Received(ControlAcknowledgement::Rejected(reason)) => {
            respond_disposition(
                connection,
                request_id,
                ControlDisposition::Rejected { reason },
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
            // The command is already accepted into the runtime input queue.
            // Report that distinction honestly, then keep the acknowledgement
            // receiver alive so `ControlRequest::application_is_live` remains
            // true and the runtime still applies it at the next turn boundary.
            respond_disposition(connection, request_id, ControlDisposition::Queued).await;
            tokio::spawn(log_late_acknowledgement(acknowledgement));
        }
    }
}

async fn log_late_acknowledgement(
    mut receiver: tokio::sync::mpsc::Receiver<ControlAcknowledgement>,
) {
    match receiver.recv().await {
        Some(ControlAcknowledgement::Applied) => {
            tracing::info!("queued control applied after acknowledgement deadline");
        }
        Some(ControlAcknowledgement::Rejected(reason)) => {
            tracing::warn!(%reason, "queued control rejected after acknowledgement deadline");
        }
        None => {
            tracing::info!("queued control dropped because the runtime input channel closed");
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

async fn respond_disposition(
    connection: &Connection<Listening>,
    request_id: i64,
    disposition: ControlDisposition,
) {
    let value = serde_json::to_value(disposition).expect("control disposition must serialize");
    let _ = connection.respond(request_id, value).await;
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

    #[tokio::test]
    async fn timed_out_request_remains_live_for_late_application() {
        let (request, mut receiver) = ControlRequest::tracked(ControlCommand::Suspend);
        let result =
            wait_for_acknowledgement(&mut receiver, std::time::Duration::from_millis(1)).await;
        assert_eq!(result, AckWait::TimedOut);

        let late = tokio::spawn(async move { receiver.recv().await });
        assert!(
            request.application_is_live(),
            "the queued runtime request must not become stale after response timeout"
        );
        request.acknowledge(ControlAcknowledgement::Applied).await;
        assert_eq!(late.await.unwrap(), Some(ControlAcknowledgement::Applied));
    }
}
