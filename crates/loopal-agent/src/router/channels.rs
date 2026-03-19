use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

/// A single message stored in a channel's history.
#[derive(Debug, Clone)]
pub struct ChannelMessage {
    pub from: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Manages pub/sub channels with subscriptions and message history.
///
/// All operations are synchronous — no locks needed because the outer
/// `MessageRouter` holds this behind a `tokio::sync::Mutex`.
pub struct ChannelStore {
    /// channel_name → set of subscribed agent names
    subscriptions: HashMap<String, HashSet<String>>,
    /// channel_name → ordered message history
    history: HashMap<String, Vec<ChannelMessage>>,
}

impl ChannelStore {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            history: HashMap::new(),
        }
    }

    /// Subscribe an agent to a channel (auto-creates the channel).
    pub fn subscribe(&mut self, channel: &str, agent_name: &str) {
        self.subscriptions
            .entry(channel.to_string())
            .or_default()
            .insert(agent_name.to_string());
        self.history.entry(channel.to_string()).or_default();
    }

    /// Unsubscribe an agent from a channel.
    pub fn unsubscribe(&mut self, channel: &str, agent_name: &str) {
        if let Some(subs) = self.subscriptions.get_mut(channel) {
            subs.remove(agent_name);
        }
    }

    /// Publish a message to a channel. Returns subscriber names excluding the sender.
    pub fn publish(&mut self, channel: &str, from: &str, content: &str) -> Vec<String> {
        // Ensure channel exists
        self.subscriptions
            .entry(channel.to_string())
            .or_default();

        let msg = ChannelMessage {
            from: from.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        };
        self.history
            .entry(channel.to_string())
            .or_default()
            .push(msg);

        self.subscriptions
            .get(channel)
            .map(|subs| {
                subs.iter()
                    .filter(|name| *name != from)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Read messages from a channel starting after the given index.
    /// Pass `0` to read from the beginning.
    pub fn read(&self, channel: &str, after_index: usize) -> Vec<ChannelMessage> {
        self.history
            .get(channel)
            .map(|msgs| {
                if after_index >= msgs.len() {
                    Vec::new()
                } else {
                    msgs[after_index..].to_vec()
                }
            })
            .unwrap_or_default()
    }

    /// List all channel names.
    pub fn list(&self) -> Vec<String> {
        self.subscriptions.keys().cloned().collect()
    }
}

impl Default for ChannelStore {
    fn default() -> Self {
        Self::new()
    }
}
