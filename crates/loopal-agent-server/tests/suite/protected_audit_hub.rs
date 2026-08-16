use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{
    PermissionDecisionAuditRequest, PermissionDecisionAuditResponse, ProtectedEffectAuditRequest,
    ProtectedEffectAuditResponse,
};
use tokio::sync::mpsc;

pub fn connection() -> Arc<Connection<Listening>> {
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_incoming) = Connection::new(agent_transport).into_listening();
    let (hub, mut incoming) = Connection::new(hub_transport).into_listening();
    tokio::spawn(async move {
        while let Some(Incoming::Request { id, method, params }) = incoming.recv().await {
            match audit_response(&method, params) {
                Some(response) => respond(&hub, id, response).await,
                None => {
                    let _ = hub.respond_error(id, -32601, "method not found").await;
                }
            }
        }
    });
    agent
}

pub fn filter(
    connection: Arc<Connection<Listening>>,
    mut incoming: mpsc::Receiver<Incoming>,
) -> mpsc::Receiver<Incoming> {
    let (forward, receiver) = mpsc::channel(32);
    tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            match message {
                Incoming::Request { id, method, params } => {
                    if let Some(response) = audit_response(&method, params.clone()) {
                        respond(&connection, id, response).await;
                    } else if forward
                        .send(Incoming::Request { id, method, params })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                message => {
                    if forward.send(message).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    receiver
}

fn audit_response(
    method: &str,
    params: serde_json::Value,
) -> Option<Result<serde_json::Value, serde_json::Error>> {
    match method {
        value if value == loopal_ipc::protocol::methods::HUB_AUDIT_PROTECTED_EFFECT.name => Some(
            serde_json::from_value::<ProtectedEffectAuditRequest>(params)
                .ok()
                .filter(|request| request.validate().is_ok())
                .map(|_| ProtectedEffectAuditResponse { recorded: true })
                .ok_or_else(invalid)
                .and_then(serde_json::to_value),
        ),
        value if value == loopal_ipc::protocol::methods::HUB_AUDIT_PERMISSION_DECISION.name => {
            Some(
                serde_json::from_value::<PermissionDecisionAuditRequest>(params)
                    .ok()
                    .filter(|request| request.validate().is_ok())
                    .map(|_| PermissionDecisionAuditResponse { recorded: true })
                    .ok_or_else(invalid)
                    .and_then(serde_json::to_value),
            )
        }
        _ => None,
    }
}

fn invalid() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "invalid audit request",
    ))
}

async fn respond(
    connection: &Connection<Listening>,
    id: i64,
    response: Result<serde_json::Value, serde_json::Error>,
) {
    match response {
        Ok(response) => {
            let _ = connection.respond(id, response).await;
        }
        Err(_) => {
            let _ = connection
                .respond_error(id, -32602, "invalid audit request")
                .await;
        }
    }
}
