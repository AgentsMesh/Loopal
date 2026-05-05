//! Control operations for SessionController — Hub mode + local (test) mode.

use std::sync::Arc;

use loopal_protocol::{ControlCommand, UserContent, UserQuestionResponse};

use loopal_agent_hub::{HubClient, LocalChannels};

/// Backend for session control operations.
pub(crate) enum ControlBackend {
    /// In-process channels — for unit tests (no real Hub).
    Local(Arc<LocalChannels>),
    /// Hub mode — all operations route through HubClient.
    Hub(Arc<HubClient>),
}

impl ControlBackend {
    /// Interrupt a specific named agent.
    pub(crate) fn interrupt_target(&self, target: &str) {
        match self {
            Self::Local(ch) => {
                tracing::info!(target, "interrupt_target: local signal");
                ch.interrupt.signal();
                ch.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
            }
            Self::Hub(client) => {
                tracing::info!(target, "interrupt_target: hub IPC");
                let client = client.clone();
                let target = target.to_string();
                tokio::spawn(async move {
                    client.interrupt_target(&target).await;
                    tracing::info!(target = %target, "interrupt_target: hub IPC sent");
                });
            }
        }
    }

    pub(crate) async fn send_control_to_agent(&self, target: &str, cmd: ControlCommand) {
        match self {
            Self::Local(ch) => {
                let _ = ch.control_tx.send(cmd).await;
            }
            Self::Hub(client) => {
                let _ = client.send_control_to(target, &cmd).await;
            }
        }
    }

    /// Route a message to a specific named agent via Hub.
    pub(crate) async fn route_to_agent(&self, target: &str, content: UserContent) {
        match self {
            Self::Local(ch) => {
                if let Some(tx) = &ch.mailbox_tx {
                    let envelope = loopal_protocol::Envelope::new(
                        loopal_protocol::MessageSource::Human,
                        target,
                        content,
                    );
                    let _ = tx.send(envelope).await;
                }
            }
            Self::Hub(client) => {
                client.send_message_to(target, content).await;
            }
        }
    }

    pub(crate) async fn respond_permission(
        &self,
        agent_name: &str,
        tool_call_id: &str,
        allow: bool,
    ) {
        match self {
            Self::Local(ch) => {
                let _ = ch.permission_tx.send(allow).await;
            }
            Self::Hub(client) => {
                client
                    .respond_permission(agent_name, tool_call_id, allow)
                    .await;
            }
        }
    }

    pub(crate) async fn respond_question(
        &self,
        agent_name: &str,
        question_id: &str,
        answers: Vec<String>,
    ) {
        match self {
            Self::Local(ch) => {
                let _ = ch
                    .question_tx
                    .send(UserQuestionResponse::answered(question_id, answers))
                    .await;
            }
            Self::Hub(client) => {
                client
                    .respond_question(agent_name, question_id, answers)
                    .await;
            }
        }
    }

    pub(crate) async fn cancel_question(&self, agent_name: &str, question_id: &str) {
        match self {
            Self::Local(ch) => {
                let _ = ch
                    .question_tx
                    .send(UserQuestionResponse::cancelled(question_id))
                    .await;
            }
            Self::Hub(client) => {
                client.cancel_question(agent_name, question_id).await;
            }
        }
    }
}
