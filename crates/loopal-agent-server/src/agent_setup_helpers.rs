//! Internal helpers split out from `agent_setup` to keep the main file
//! small and each helper single-purpose.

use std::sync::Arc;

use loopal_config::ResolvedConfig;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_runtime::frontend::traits::AgentFrontend;

use crate::params::StartParams;

/// Spawn the sub-agent lifecycle forwarder. Listens for `SubAgentSpawned`
/// events on `event_rx` and forwards them to the root frontend so the TUI
/// can auto-attach to newly spawned child agents.
///
/// Returns the sender side of the channel that `AgentShared` should use
/// as `parent_event_tx` for the root agent.
pub fn spawn_sub_agent_forwarder(
    frontend: Arc<dyn AgentFrontend>,
) -> tokio::sync::mpsc::Sender<AgentEvent> {
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if matches!(event.payload, AgentEventPayload::SubAgentSpawned { .. }) {
                let _ = frontend.emit(event.payload).await;
            }
        }
    });
    event_tx
}

/// Compose the initial message list for the agent loop.
///
/// Starts from any resumed messages, appends a deserialized fork context
/// (when the start params request it and there is no resume in progress),
/// and finally appends the user prompt wrapped with fork boilerplate if
/// fork was applied.
pub fn build_initial_messages(
    resume_messages: Vec<loopal_message::Message>,
    start: &StartParams,
) -> Vec<loopal_message::Message> {
    let mut messages = resume_messages;
    let mut has_fork = false;
    if let Some(ref fc_value) = start.fork_context
        && start.resume.is_none()
    {
        match serde_json::from_value::<Vec<loopal_message::Message>>(fc_value.clone()) {
            Ok(fork_msgs) => {
                messages.extend(fork_msgs);
                has_fork = true;
            }
            Err(e) => tracing::warn!("fork context deserialization failed, skipping: {e}"),
        }
    }
    if let Some(prompt) = &start.prompt {
        let text = if has_fork {
            format!("{}\n\n{prompt}", loopal_context::fork::FORK_BOILERPLATE)
        } else {
            prompt.to_string()
        };
        messages.push(loopal_message::Message::user(&text));
    }
    messages
}

/// Collect runtime feature tags for the system prompt preamble.
pub fn collect_feature_tags(config: &ResolvedConfig, has_memory_channel: bool) -> Vec<String> {
    let mut features = Vec::new();
    if config.settings.memory.enabled && has_memory_channel {
        features.push("memory".into());
    }
    if !config.settings.hooks.is_empty() {
        features.push("hooks".into());
    }
    features.push("subagent".into());
    if !config.settings.output_style.is_empty() {
        features.push(format!("style_{}", config.settings.output_style));
    }
    features
}
