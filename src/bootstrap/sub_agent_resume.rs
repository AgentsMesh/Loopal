//! Sub-agent session persistence and resume.
//!
//! `load_sub_agent_histories` reads sub-agent refs persisted by the Hub
//! process and seeds TUI display state on resume.
//!
//! `hub_persist_watcher` runs in the Hub-only process: it subscribes to
//! the Hub's event broadcast and writes a `SubAgentRef` to disk every
//! time a sub-agent is spawned.

use loopal_protocol::{AgentEventPayload, project_messages};
use loopal_session::ROOT_AGENT;
use loopal_storage::SubAgentRef;
use tokio::sync::broadcast;
use tracing::warn;

use loopal_protocol::AgentEvent;

/// Load sub-agent conversation histories from their persisted sessions.
pub fn load_sub_agent_histories(
    app: &mut loopal_tui::app::App,
    session: &loopal_storage::Session,
    session_manager: &loopal_runtime::SessionManager,
) {
    for sub_ref in &session.sub_agents {
        let messages = match session_manager.load_messages(&sub_ref.session_id) {
            Ok(msgs) => msgs,
            Err(e) => {
                warn!(
                    agent = %sub_ref.name, sid = %sub_ref.session_id,
                    error = %e, "failed to load sub-agent history, skipping"
                );
                continue;
            }
        };
        if messages.is_empty() {
            continue;
        }
        app.load_sub_agent_history(
            &sub_ref.name,
            &sub_ref.session_id,
            sub_ref.parent.as_deref(),
            sub_ref.model.as_deref(),
            project_messages(&messages),
        );
    }
}

/// Hub-side persistence watcher.
///
/// Subscribes to the Hub event broadcast and writes a `SubAgentRef` to
/// disk every time a sub-agent is spawned. Tracks `root_session_id` so
/// that mid-session `/resume` of the root agent (which changes its
/// session id) routes subsequent sub-agents to the new root.
pub async fn hub_persist_watcher(
    mut events: broadcast::Receiver<AgentEvent>,
    initial_root_session_id: String,
) {
    let mut root_session_id = initial_root_session_id;
    let session_manager = match loopal_runtime::SessionManager::new() {
        Ok(sm) => sm,
        Err(e) => {
            warn!(error = %e, "hub persister: SessionManager init failed; sub-agents will not be persisted");
            return;
        }
    };
    loop {
        let event = match events.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Closed) => return,
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!(skipped, "hub persister: event stream lagged");
                continue;
            }
        };
        match &event.payload {
            AgentEventPayload::SessionResumed { session_id, .. } => {
                let is_root = event
                    .agent_name
                    .as_ref()
                    .map(|q| q.agent.as_str())
                    .unwrap_or("")
                    == ROOT_AGENT;
                if is_root && !session_id.is_empty() {
                    root_session_id = session_id.clone();
                }
            }
            AgentEventPayload::SubAgentSpawned {
                name,
                session_id: Some(sid),
                parent,
                model,
                ..
            } => {
                let sub_ref = SubAgentRef {
                    name: name.clone(),
                    session_id: sid.clone(),
                    parent: parent.as_ref().map(|p| p.to_string()),
                    model: model.clone(),
                };
                if let Err(e) = session_manager.add_sub_agent(&root_session_id, sub_ref) {
                    warn!(
                        agent = %name, error = %e,
                        "hub persister: failed to persist sub-agent ref"
                    );
                }
            }
            _ => {}
        }
    }
}
