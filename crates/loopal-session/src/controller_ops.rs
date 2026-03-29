//! Control operations for SessionController — Hub mode + local (test) mode.

use std::sync::Arc;

use loopal_protocol::{ControlCommand, UserContent, UserQuestionResponse};

use crate::inbox::try_forward_inbox;
use crate::state::SessionState;
use loopal_agent_hub::{HubClient, LocalChannels};

/// Backend for session control operations.
pub(crate) enum ControlBackend {
    /// In-process channels — for unit tests (no real Hub).
    Local(Arc<LocalChannels>),
    /// Hub mode — all operations route through HubClient.
    Hub(Arc<HubClient>),
}

impl ControlBackend {
    pub(crate) fn interrupt(&self) {
        match self {
            Self::Local(ch) => {
                ch.interrupt.signal();
                ch.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
            }
            Self::Hub(client) => {
                let client = client.clone();
                tokio::spawn(async move {
                    client.interrupt().await;
                });
            }
        }
    }

    pub(crate) async fn send_control(&self, cmd: ControlCommand) {
        match self {
            Self::Local(ch) => {
                let _ = ch.control_tx.send(cmd).await;
            }
            Self::Hub(client) => {
                let _ = client.send_control(&cmd).await;
            }
        }
    }

    pub(crate) async fn send_message(&self, content: UserContent) {
        match self {
            Self::Local(ch) => {
                if let Some(tx) = &ch.mailbox_tx {
                    let envelope = loopal_protocol::Envelope::new(
                        loopal_protocol::MessageSource::Human,
                        "main",
                        content,
                    );
                    let _ = tx.send(envelope).await;
                }
            }
            Self::Hub(client) => {
                client.send_message(content).await;
            }
        }
    }

    /// Approve a pending permission request.
    ///
    /// In Hub mode, sends the response via IPC using the relay request ID.
    pub(crate) async fn approve_permission(&self, relay_request_id: Option<i64>) {
        match self {
            Self::Local(ch) => {
                let _ = ch.permission_tx.send(true).await;
            }
            Self::Hub(client) => {
                if let Some(id) = relay_request_id {
                    client.respond_permission(id, true).await;
                }
            }
        }
    }

    /// Deny a pending permission request.
    pub(crate) async fn deny_permission(&self, relay_request_id: Option<i64>) {
        match self {
            Self::Local(ch) => {
                let _ = ch.permission_tx.send(false).await;
            }
            Self::Hub(client) => {
                if let Some(id) = relay_request_id {
                    client.respond_permission(id, false).await;
                }
            }
        }
    }

    /// Answer a pending question.
    pub(crate) async fn answer_question(
        &self,
        answers: Vec<String>,
        relay_request_id: Option<i64>,
    ) {
        match self {
            Self::Local(ch) => {
                let _ = ch.question_tx.send(UserQuestionResponse { answers }).await;
            }
            Self::Hub(client) => {
                if let Some(id) = relay_request_id {
                    client.respond_question(id, answers).await;
                }
            }
        }
    }
}

/// Forward a pending inbox message to the agent.
pub(crate) fn try_forward_from_inbox(state: &mut SessionState) -> Option<UserContent> {
    try_forward_inbox(state)
}
