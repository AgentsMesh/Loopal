use std::collections::HashMap;

use tokio::sync::{Mutex, mpsc};

use loopal_types::envelope::Envelope;
use loopal_types::event::{AgentEvent, AgentEventPayload};

use super::channels::ChannelStore;

/// Unified message router for the three-plane architecture.
///
/// Replaces the separate `MessageBus` + `ChannelHub` with a single entry
/// point for all inter-agent communication. Each `route()` or `broadcast()`
/// also emits a `MessageRouted` event to the observation plane.
pub struct MessageRouter {
    mailboxes: Mutex<HashMap<String, mpsc::Sender<Envelope>>>,
    observation_tx: mpsc::Sender<AgentEvent>,
    channels: Mutex<ChannelStore>,
}

impl MessageRouter {
    pub fn new(observation_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            mailboxes: Mutex::new(HashMap::new()),
            observation_tx,
            channels: Mutex::new(ChannelStore::new()),
        }
    }

    /// Register a named mailbox. Returns error if the name is already taken.
    pub async fn register(
        &self,
        name: &str,
        tx: mpsc::Sender<Envelope>,
    ) -> Result<(), String> {
        let mut map = self.mailboxes.lock().await;
        if map.contains_key(name) {
            return Err(format!("mailbox '{}' already registered", name));
        }
        map.insert(name.to_string(), tx);
        Ok(())
    }

    /// Remove a mailbox registration.
    pub async fn unregister(&self, name: &str) {
        self.mailboxes.lock().await.remove(name);
    }

    /// Deliver an envelope to its target mailbox and emit a MessageRouted event.
    pub async fn route(&self, envelope: Envelope) -> Result<(), String> {
        let tx = {
            let map = self.mailboxes.lock().await;
            map.get(&envelope.target).cloned()
        };
        let tx = tx.ok_or_else(|| {
            format!("no mailbox registered for '{}'", envelope.target)
        })?;

        let event = build_routed_event(&envelope);
        tx.send(envelope)
            .await
            .map_err(|e| format!("send failed: {e}"))?;

        // Best-effort observation — don't fail the route if event send fails
        let _ = self.observation_tx.send(event).await;
        Ok(())
    }

    /// Deliver an envelope to all registered mailboxes except the excluded one.
    /// Returns the list of agent names that received the message.
    pub async fn broadcast(
        &self,
        envelope: Envelope,
        exclude: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let targets: Vec<(String, mpsc::Sender<Envelope>)> = {
            let map = self.mailboxes.lock().await;
            map.iter()
                .filter(|(name, _)| {
                    exclude != Some(name.as_str())
                })
                .map(|(name, tx)| (name.clone(), tx.clone()))
                .collect()
        };

        let mut delivered = Vec::new();
        for (name, tx) in targets {
            let mut env = envelope.clone();
            env.target = name.clone();

            let event = build_routed_event(&env);
            if tx.send(env).await.is_ok() {
                delivered.push(name);
                let _ = self.observation_tx.send(event).await;
            }
        }
        Ok(delivered)
    }

    // --- Channel delegation methods ---

    /// Subscribe an agent to a pub/sub channel.
    pub async fn subscribe(&self, channel: &str, agent_name: &str) {
        self.channels.lock().await.subscribe(channel, agent_name);
    }

    /// Unsubscribe an agent from a pub/sub channel.
    pub async fn unsubscribe(&self, channel: &str, agent_name: &str) {
        self.channels.lock().await.unsubscribe(channel, agent_name);
    }

    /// Publish to a channel. Returns subscriber names excluding sender.
    pub async fn publish(
        &self,
        channel: &str,
        from: &str,
        content: &str,
    ) -> Vec<String> {
        self.channels.lock().await.publish(channel, from, content)
    }

    /// Read channel history after a given index.
    pub async fn read_channel(
        &self,
        channel: &str,
        after_index: usize,
    ) -> Vec<super::ChannelMessage> {
        self.channels.lock().await.read(channel, after_index)
    }

    /// List all pub/sub channels.
    pub async fn list_channels(&self) -> Vec<String> {
        self.channels.lock().await.list()
    }
}

fn build_routed_event(envelope: &Envelope) -> AgentEvent {
    AgentEvent::root(AgentEventPayload::MessageRouted {
        source: envelope.source.label(),
        target: envelope.target.clone(),
        content_preview: envelope.content_preview().to_string(),
    })
}
