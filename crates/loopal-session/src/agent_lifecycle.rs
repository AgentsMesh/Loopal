//! Agent lifecycle: idle state transitions, error recovery, topology registration, display helpers.

use loopal_protocol::{AgentStatus, UserContent};

use crate::inbox::try_forward_inbox;
use crate::state::{ROOT_AGENT, SessionState};

/// Shared idle handling for AwaitingInput / Finished / Interrupted.
pub(crate) fn handle_idle(
    state: &mut SessionState,
    name: &str,
    status: AgentStatus,
) -> Option<UserContent> {
    let agent = state.agents.get_mut(name)?;
    agent.conversation.flush_streaming();
    agent.conversation.end_turn();
    if status != AgentStatus::Finished {
        agent.conversation.turn_count += 1;
        agent.observable.turn_count += 1;
    }
    agent.conversation.agent_idle = true;
    agent.conversation.retry_banner = None;
    agent.observable.status = status;
    // Auto-return to root when the viewed agent finishes
    if status == AgentStatus::Finished && state.active_view == name {
        state.active_view = ROOT_AGENT.to_string();
    }
    // Truncate completed sub-agent conversation to bound memory growth
    truncate_if_done(state, name);
    if state.active_view == name {
        return try_forward_inbox(state);
    }
    None
}

/// Register a newly spawned agent with parent/child topology.
pub(crate) fn register_spawned_agent(
    state: &mut SessionState,
    name: &str,
    parent: Option<&str>,
    model: Option<&str>,
    session_id: Option<&str>,
) {
    let agent = state.agents.entry(name.to_string()).or_default();
    agent.parent = parent.map(String::from);
    agent.session_id = session_id.map(String::from);
    if let Some(m) = model {
        agent.observable.model = m.to_string();
    }
    if let Some(p) = parent
        && let Some(parent_agent) = state.agents.get_mut(p)
    {
        let child_name = name.to_string();
        if !parent_agent.children.contains(&child_name) {
            parent_agent.children.push(child_name);
        }
    }
}

/// Post-event cleanup: error recovery + parent tool progress sync.
pub(crate) fn post_event_cleanup(state: &mut SessionState, name: &str, sync_parent: bool) {
    auto_return_on_error(state, name);
    if sync_parent && name != ROOT_AGENT {
        sync_parent_tool_progress(state, name);
    }
}

/// Auto-return to root view when the viewed agent enters Error status.
/// Also truncates the conversation to bound memory.
pub(crate) fn auto_return_on_error(state: &mut SessionState, name: &str) {
    if state.active_view == name
        && state
            .agents
            .get(name)
            .is_some_and(|a| a.observable.status == AgentStatus::Error)
    {
        state.active_view = ROOT_AGENT.to_string();
    }
    truncate_if_done(state, name);
}

/// Truncate a non-root agent's conversation if it's Finished or Error.
/// Keeps the last 20 messages to bound memory in long sessions.
fn truncate_if_done(state: &mut SessionState, name: &str) {
    const MAX_DONE_MSGS: usize = 20;
    if name == ROOT_AGENT {
        return;
    }
    let Some(agent) = state.agents.get_mut(name) else {
        return;
    };
    if !matches!(
        agent.observable.status,
        AgentStatus::Finished | AgentStatus::Error
    ) {
        return;
    }
    let msgs = &mut agent.conversation.messages;
    if msgs.len() > MAX_DONE_MSGS {
        let start = msgs.len() - MAX_DONE_MSGS;
        *msgs = msgs.split_off(start);
    }
}

/// Propagate sub-agent's last tool activity to the parent's Agent tool call.
///
/// Syncs `observable.last_tool` into the parent's `SessionToolCall.progress_tail`
/// so the TUI content area can display live sub-agent status.
pub(crate) fn sync_parent_tool_progress(state: &mut SessionState, child_name: &str) {
    // Phase 1: read child state (shared borrow released before phase 2).
    let (parent_name, status_text) = {
        let Some(agent) = state.agents.get(child_name) else {
            return;
        };
        let parent = match &agent.parent {
            Some(p) => p.clone(),
            None => return,
        };
        let text = agent.observable.last_tool.clone().unwrap_or_default();
        (parent, text)
    };

    // Phase 2: find the matching Agent tool call in parent's conversation.
    let Some(parent) = state.agents.get_mut(&parent_name) else {
        return;
    };
    for msg in parent.conversation.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.name == "Agent" && tc.status.is_active() {
                let matches = tc
                    .tool_input
                    .as_ref()
                    .and_then(|i| i.get("name"))
                    .and_then(|v| v.as_str())
                    == Some(child_name);
                if matches {
                    tc.progress_tail = if status_text.is_empty() {
                        None
                    } else {
                        Some(status_text)
                    };
                    return;
                }
            }
        }
    }
}

/// Extract the most informative parameter from a tool call for display.
pub(crate) fn extract_key_param(tool_name: &str, input: &serde_json::Value) -> String {
    let key = match tool_name {
        "Read" | "Write" | "Edit" | "MultiEdit" => "file_path",
        "Bash" => "command",
        "Grep" | "Glob" => "pattern",
        _ => return tool_name.to_string(),
    };
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| {
            if s.len() > 40 {
                let t: String = s.chars().take(37).collect();
                format!("{tool_name}({t}...)")
            } else {
                format!("{tool_name}({s})")
            }
        })
        .unwrap_or_else(|| tool_name.to_string())
}
