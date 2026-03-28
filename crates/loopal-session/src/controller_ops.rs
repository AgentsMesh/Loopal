//! Control operations for SessionController — Hub mode + local (test) mode.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{ControlCommand, Envelope, MessageSource, UserContent, UserQuestionResponse};
use serde_json::json;

use crate::inbox::try_forward_inbox;
use crate::state::SessionState;
use loopal_agent_hub::LocalChannels;

/// Backend for session control operations.
pub(crate) enum ControlBackend {
    /// In-process channels — for unit tests (no real Hub).
    Local(Arc<LocalChannels>),
    /// Hub mode — all operations route through Hub TCP Connection.
    Hub(Arc<Connection>),
}

impl ControlBackend {
    pub(crate) fn interrupt(&self) {
        match self {
            Self::Local(ch) => {
                ch.interrupt.signal();
                ch.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
            }
            Self::Hub(conn) => {
                let conn = conn.clone();
                tokio::spawn(async move {
                    let _ = conn
                        .send_request(methods::HUB_INTERRUPT.name, json!({"target": "main"}))
                        .await;
                });
            }
        }
    }

    pub(crate) async fn send_control(&self, cmd: ControlCommand) {
        match self {
            Self::Local(ch) => {
                let _ = ch.control_tx.send(cmd).await;
            }
            Self::Hub(conn) => {
                let params = json!({
                    "target": "main",
                    "command": serde_json::to_value(&cmd).unwrap_or_default(),
                });
                let _ = conn.send_request(methods::HUB_CONTROL.name, params).await;
            }
        }
    }

    pub(crate) async fn send_message(&self, content: UserContent) {
        let envelope = Envelope::new(MessageSource::Human, "main", content);
        match self {
            Self::Local(ch) => {
                if let Some(tx) = &ch.mailbox_tx {
                    let _ = tx.send(envelope).await;
                }
            }
            Self::Hub(conn) => {
                if let Ok(params) = serde_json::to_value(&envelope) {
                    let _ = conn.send_request(methods::HUB_ROUTE.name, params).await;
                }
            }
        }
    }

    pub(crate) async fn approve_permission(&self) {
        match self {
            Self::Local(ch) => {
                let _ = ch.permission_tx.send(true).await;
            }
            Self::Hub(_) => {
                // Hub mode: permission is handled via IPC request/response flow.
            }
        }
    }

    pub(crate) async fn deny_permission(&self) {
        match self {
            Self::Local(ch) => {
                let _ = ch.permission_tx.send(false).await;
            }
            Self::Hub(_) => {}
        }
    }

    pub(crate) async fn answer_question(&self, answers: Vec<String>) {
        match self {
            Self::Local(ch) => {
                let _ = ch.question_tx.send(UserQuestionResponse { answers }).await;
            }
            Self::Hub(_) => {}
        }
    }
}

/// Forward a pending inbox message to the agent.
pub(crate) fn try_forward_from_inbox(state: &mut SessionState) -> Option<UserContent> {
    try_forward_inbox(state)
}
