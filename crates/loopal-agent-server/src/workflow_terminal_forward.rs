use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::jsonrpc;
use loopal_protocol::{WorkflowTerminalDisposition, WorkflowTerminalNotification};
use loopal_runtime::agent_input::{AgentInput, WorkflowTerminalRequest};

use crate::session_hub::SharedSession;
use crate::workflow_terminal_pending::WorkflowTerminalClaim;

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
    forward_with_timeout(
        request_id,
        params,
        session,
        connection,
        loopal_protocol::DEFAULT_WORKFLOW_TERMINAL_APPLICATION_TIMEOUT,
    )
    .await;
}

async fn forward_with_timeout(
    request_id: i64,
    params: serde_json::Value,
    session: &SharedSession,
    connection: &Connection<Listening>,
    application_timeout: std::time::Duration,
) {
    let notification = match serde_json::from_value::<WorkflowTerminalNotification>(params) {
        Ok(notification) => notification,
        Err(error) => {
            reject_rpc(
                connection,
                request_id,
                format!("invalid workflow terminal: {error}"),
            )
            .await;
            return;
        }
    };
    if let Err(error) = notification.validate() {
        reject_rpc(connection, request_id, error.to_string()).await;
        return;
    }
    if notification.delivery_id.session_id != session.session_id {
        respond(
            connection,
            request_id,
            WorkflowTerminalDisposition::Rejected {
                reason: "workflow terminal delivery targets a different session".into(),
            },
        )
        .await;
        return;
    }

    let (request, mut acknowledgement) = WorkflowTerminalRequest::tracked(notification.clone());
    let delivery_id = notification.delivery_id.clone();
    let payload_digest = notification.payload_digest();
    match session
        .claim_workflow_terminal(&notification, acknowledgement.clone())
        .await
    {
        WorkflowTerminalClaim::New => {}
        WorkflowTerminalClaim::Pending => {
            respond(connection, request_id, WorkflowTerminalDisposition::Queued).await;
            return;
        }
        WorkflowTerminalClaim::Completed(disposition) => {
            respond(connection, request_id, disposition).await;
            return;
        }
        WorkflowTerminalClaim::Conflict => {
            respond(
                connection,
                request_id,
                WorkflowTerminalDisposition::Rejected {
                    reason: "workflow terminal delivery id conflicts with pending payload".into(),
                },
            )
            .await;
            return;
        }
        WorkflowTerminalClaim::Full => {
            respond(
                connection,
                request_id,
                WorkflowTerminalDisposition::Retryable {
                    reason: "too many pending workflow terminal deliveries".into(),
                },
            )
            .await;
            return;
        }
    }
    if session
        .input_tx
        .send(AgentInput::WorkflowTerminal(request))
        .await
        .is_err()
    {
        session
            .discard_workflow_terminal(&delivery_id, &payload_digest, &acknowledgement)
            .await;
        reject_rpc(connection, request_id, "agent input channel is closed").await;
        return;
    }
    match wait_for_acknowledgement(&mut acknowledgement, application_timeout).await {
        AckWait::Received(disposition) => {
            if matches!(
                disposition,
                WorkflowTerminalDisposition::Queued | WorkflowTerminalDisposition::Retryable { .. }
            ) {
                session
                    .discard_workflow_terminal(&delivery_id, &payload_digest, &acknowledgement)
                    .await;
            }
            respond(connection, request_id, disposition).await;
        }
        AckWait::Closed => {
            session
                .discard_workflow_terminal(&delivery_id, &payload_digest, &acknowledgement)
                .await;
            reject_rpc(
                connection,
                request_id,
                "workflow terminal application channel closed before acknowledgement",
            )
            .await
        }
        AckWait::TimedOut => {
            respond(connection, request_id, WorkflowTerminalDisposition::Queued).await;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum AckWait {
    Received(WorkflowTerminalDisposition),
    Closed,
    TimedOut,
}

async fn wait_for_acknowledgement(
    receiver: &mut tokio::sync::watch::Receiver<Option<WorkflowTerminalDisposition>>,
    timeout: std::time::Duration,
) -> AckWait {
    if let Some(disposition) = receiver.borrow().clone() {
        return AckWait::Received(disposition);
    }
    match tokio::time::timeout(timeout, receiver.changed()).await {
        Ok(Ok(())) => receiver
            .borrow()
            .clone()
            .map_or(AckWait::Closed, AckWait::Received),
        Ok(Err(_)) => AckWait::Closed,
        Err(_) => AckWait::TimedOut,
    }
}

async fn respond(
    connection: &Connection<Listening>,
    request_id: i64,
    disposition: WorkflowTerminalDisposition,
) {
    let value = serde_json::to_value(disposition).expect("terminal disposition serializes");
    let _ = connection.respond(request_id, value).await;
}

async fn reject_rpc(connection: &Connection<Listening>, request_id: i64, reason: impl AsRef<str>) {
    let _ = connection
        .respond_error(request_id, jsonrpc::INVALID_REQUEST, reason.as_ref())
        .await;
}

#[cfg(test)]
#[path = "workflow_terminal_forward/tests.rs"]
mod tests;
