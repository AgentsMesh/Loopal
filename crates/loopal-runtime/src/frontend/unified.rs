use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};
use tokio_util::sync::CancellationToken;

use loopal_types::agent_input::AgentInput;
use loopal_types::control::ControlCommand;
use loopal_types::envelope::Envelope;
use loopal_types::error::Result;
use loopal_types::event::{AgentEvent, AgentEventPayload};
use loopal_types::frontend::{AgentFrontend, EventEmitter};
use loopal_types::permission::PermissionDecision;

use super::permission_handler::PermissionHandler;
use super::emitter::ChannelEventEmitter;

/// Unified frontend merging root (channel-based) and sub-agent (autopilot) behaviour.
///
/// Consumes Envelope + ControlCommand channels and returns `AgentInput`
/// directly — no intermediate `UserCommand` adaptation.
///
/// - Root agent:  `agent_name = None`, uses `TuiPermissionHandler`
/// - Sub-agent:   `agent_name = Some(name)`, uses `AutoDenyHandler`
pub struct UnifiedFrontend {
    agent_name: Option<String>,
    event_tx: mpsc::Sender<AgentEvent>,
    mailbox_rx: Mutex<mpsc::Receiver<Envelope>>,
    control_rx: Mutex<mpsc::Receiver<ControlCommand>>,
    cancel_token: Option<CancellationToken>,
    permission_handler: Box<dyn PermissionHandler>,
}

impl UnifiedFrontend {
    pub fn new(
        agent_name: Option<String>,
        event_tx: mpsc::Sender<AgentEvent>,
        mailbox_rx: mpsc::Receiver<Envelope>,
        control_rx: mpsc::Receiver<ControlCommand>,
        cancel_token: Option<CancellationToken>,
        permission_handler: Box<dyn PermissionHandler>,
    ) -> Self {
        Self {
            agent_name,
            event_tx,
            mailbox_rx: Mutex::new(mailbox_rx),
            control_rx: Mutex::new(control_rx),
            cancel_token,
            permission_handler,
        }
    }
}

#[async_trait]
impl AgentFrontend for UnifiedFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        if self.agent_name.is_some() {
            // Sub-agent: best-effort
            let _ = self.event_tx.send(event).await;
            Ok(())
        } else {
            // Root: propagate send errors
            self.event_tx.send(event).await.map_err(|e| {
                warn!(error = %e, "event channel closed");
                loopal_types::error::LoopalError::Other(
                    "event channel closed".into(),
                )
            })
        }
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        let mut mbox = self.mailbox_rx.lock().await;
        let mut ctrl = self.control_rx.lock().await;
        if let Some(ref token) = self.cancel_token {
            tokio::select! {
                env = mbox.recv() => env.map(AgentInput::Message),
                cmd = ctrl.recv() => cmd.map(AgentInput::Control),
                () = token.cancelled() => {
                    info!("cancellation triggered in unified frontend");
                    None
                }
            }
        } else {
            tokio::select! {
                env = mbox.recv() => env.map(AgentInput::Message),
                cmd = ctrl.recv() => cmd.map(AgentInput::Control),
            }
        }
    }

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision {
        self.permission_handler.decide(id, name, input).await
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(ChannelEventEmitter::new(
            self.event_tx.clone(),
            self.agent_name.clone(),
        ))
    }

    async fn drain_pending(&self) -> Vec<Envelope> {
        let mut rx = self.mailbox_rx.lock().await;
        let mut envelopes = Vec::new();
        while let Ok(env) = rx.try_recv() {
            envelopes.push(env);
        }
        envelopes
    }
}
