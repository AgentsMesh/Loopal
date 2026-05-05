use loopal_protocol::{CronJobSnapshot, McpServerSnapshot, TaskSnapshot};

use crate::state::SessionViewState;

pub(super) fn tasks_changed(state: &mut SessionViewState, tasks: &[TaskSnapshot]) -> bool {
    state.tasks = tasks.to_vec();
    true
}

pub(super) fn crons_changed(state: &mut SessionViewState, crons: &[CronJobSnapshot]) -> bool {
    state.crons = crons.to_vec();
    true
}

pub(super) fn mcp_status(state: &mut SessionViewState, servers: &[McpServerSnapshot]) -> bool {
    state.mcp_status = Some(servers.to_vec());
    true
}

pub(super) fn sub_agent_spawned(state: &mut SessionViewState, name: &str) -> bool {
    if state.agent.children.iter().any(|n| n == name) {
        return false;
    }
    state.agent.children.push(name.to_string());
    true
}

pub(super) fn session_resumed(state: &mut SessionViewState, session_id: &str) -> bool {
    state.agent.session_id = Some(session_id.to_string());
    state.tasks.clear();
    state.crons.clear();
    state.bg_tasks.clear();
    true
}
