//! Sub-agent session persistence and resume.
//!
//! Records sub-agent session references in the root session metadata so that
//! sub-agent conversation history can be restored when resuming a session.

use loopal_protocol::project_messages;
use loopal_session::SessionController;
use loopal_storage::SubAgentRef;
use tracing::warn;

/// Load sub-agent conversation histories from their persisted sessions.
pub fn load_sub_agent_histories(
    session_ctrl: &SessionController,
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
        let display_msgs = project_messages(&messages)
            .into_iter()
            .map(loopal_session::into_session_message)
            .collect();
        let mut state = session_ctrl.lock();
        let agent = state.agents.entry(sub_ref.name.clone()).or_default();
        agent.parent = sub_ref.parent.clone();
        agent.session_id = Some(sub_ref.session_id.clone());
        if let Some(ref m) = sub_ref.model {
            agent.observable.model = m.clone();
        }
        agent.conversation.messages = display_msgs;
        agent.conversation.agent_idle = true;
        agent.observable.status = loopal_protocol::AgentStatus::Finished;
        // Register as child of parent
        if let Some(ref parent_name) = sub_ref.parent {
            let child_name = sub_ref.name.clone();
            if let Some(parent_agent) = state.agents.get_mut(parent_name) {
                if !parent_agent.children.contains(&child_name) {
                    parent_agent.children.push(child_name);
                }
            }
        }
    }
}

/// Background loop: drain pending sub-agent refs and persist to disk.
///
/// Runs until the tokio runtime shuts down (when `run_tui` returns).
/// Holds an `Arc` clone of `SessionController`, so it won't outlive the state.
pub async fn persist_sub_agent_refs_loop(ctrl: SessionController) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        interval.tick().await;
        if let Some((root_id, refs)) = ctrl.drain_pending_sub_agent_refs() {
            let Ok(mgr) = loopal_runtime::SessionManager::new() else {
                continue;
            };
            for r in refs {
                let sub_ref = SubAgentRef {
                    name: r.name.clone(),
                    session_id: r.session_id.clone(),
                    parent: r.parent.clone(),
                    model: r.model.clone(),
                };
                if let Err(e) = mgr.add_sub_agent(&root_id, sub_ref) {
                    tracing::warn!(
                        agent = %r.name, error = %e,
                        "failed to persist sub-agent ref"
                    );
                }
            }
        }
    }
}
