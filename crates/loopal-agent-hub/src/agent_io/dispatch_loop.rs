use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, PROTOCOL_ERROR_REASON, TRANSPORT_ERROR_REASON};

use crate::hub::Hub;
use crate::types::AgentExecutionRef;

use super::event_forward::forward_agent_event;
use super::request_dispatch::handle_request;

pub async fn agent_io_loop(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    agent_name: String,
) -> AgentCompletion {
    let execution = hub
        .lock()
        .await
        .registry
        .execution_for_connection(&agent_name, &conn);
    let Some(execution) = execution else {
        return AgentCompletion::new(
            PROTOCOL_ERROR_REASON,
            Some("agent IO loop requires an active connection lease".into()),
        );
    };
    agent_io_loop_exact(hub, dispatcher, conn, rx, agent_name, execution).await
}

pub(crate) async fn agent_io_loop_exact(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    conn: Arc<Connection<Listening>>,
    mut rx: tokio::sync::mpsc::Receiver<Incoming>,
    agent_name: String,
    execution: AgentExecutionRef,
) -> AgentCompletion {
    info!(agent = %agent_name, "agent IO loop started");

    while let Some(message) = rx.recv().await {
        match message {
            Incoming::Notification { method, params } => {
                if method == methods::AGENT_COMPLETED.name {
                    match serde_json::from_value::<AgentCompletion>(params) {
                        Ok(completion) => {
                            info!(
                                agent = %agent_name,
                                reason = %completion.reason,
                                has_result = completion.result.is_some(),
                                "received agent/completed"
                            );
                            let (redaction_seed, result_limit) = {
                                let hub = hub.lock().await;
                                (
                                    hub.final_sink_redaction_seed(),
                                    hub.registry.completion_result_limit(&execution),
                                )
                            };
                            return crate::completion_guard::guard_with_result_limit(
                                completion,
                                &redaction_seed,
                                result_limit,
                            );
                        }
                        Err(error) => {
                            warn!(agent = %agent_name, %error, "malformed agent/completed");
                            return AgentCompletion::new(
                                PROTOCOL_ERROR_REASON,
                                Some(format!("malformed agent/completed: {error}")),
                            );
                        }
                    }
                } else if method == methods::AGENT_EVENT.name {
                    if let Err(error) = forward_agent_event(&hub, &conn, &agent_name, params).await
                    {
                        warn!(agent = %agent_name, %error, "authoritative event transport failed");
                        return AgentCompletion::new(TRANSPORT_ERROR_REASON, Some(error));
                    }
                } else if method == methods::REQUEST_CANCEL.name {
                    let Some(request_id) = params.get("id").and_then(|value| value.as_i64()) else {
                        warn!(agent = %agent_name, "malformed request cancellation ignored");
                        continue;
                    };
                    crate::pending_relay::cancel_pending_request(
                        &hub,
                        &agent_name,
                        &conn,
                        request_id,
                    )
                    .await;
                }
            }
            Incoming::Request { id, method, params } => {
                handle_request(
                    &hub,
                    &dispatcher,
                    &conn,
                    &agent_name,
                    &execution,
                    id,
                    method,
                    params,
                )
                .await;
            }
        }
    }
    AgentCompletion::new(
        TRANSPORT_ERROR_REASON,
        Some("agent transport closed before agent/completed".into()),
    )
}
