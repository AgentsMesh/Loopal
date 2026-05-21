use loopal_protocol::{CronJobSnapshot, McpServerSnapshot, TaskSnapshot, ThreadGoal};

use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn tasks_changed(
    state: &mut SessionViewState,
    tasks: &[TaskSnapshot],
) -> MutationEffect {
    state.tasks = tasks.to_vec();
    MutationEffect::Mutated
}

pub(super) fn crons_changed(
    state: &mut SessionViewState,
    crons: &[CronJobSnapshot],
) -> MutationEffect {
    state.crons = crons.to_vec();
    MutationEffect::Mutated
}

pub(super) fn mcp_status(
    state: &mut SessionViewState,
    servers: &[McpServerSnapshot],
) -> MutationEffect {
    state.mcp_status = Some(servers.to_vec());
    MutationEffect::Mutated
}

pub(super) fn sub_agent_spawned(state: &mut SessionViewState, name: &str) -> MutationEffect {
    if state.agent.children.iter().any(|n| n == name) {
        return MutationEffect::NoOp;
    }
    state.agent.children.push(name.to_string());
    MutationEffect::Mutated
}

pub(super) fn session_resumed(state: &mut SessionViewState, session_id: &str) -> MutationEffect {
    state.agent.session_id = Some(session_id.to_string());
    state.agent.conversation.clear_history();
    state.tasks.clear();
    state.crons.clear();
    state.bg_tasks.clear();
    state.thread_goal = None;
    // Clear stale Hub-health snapshot; the next poller tick will re-sync.
    state.hub_degraded_since_ms = None;
    MutationEffect::Mutated
}

pub(super) fn thread_goal_updated(
    state: &mut SessionViewState,
    goal: &Option<ThreadGoal>,
) -> MutationEffect {
    if state.thread_goal.as_ref() == goal.as_ref() {
        return MutationEffect::NoOp;
    }
    state.thread_goal = goal.clone();
    MutationEffect::Mutated
}

pub(super) fn hub_degraded(state: &mut SessionViewState, since_unix_ms: u64) -> MutationEffect {
    if state.hub_degraded_since_ms == Some(since_unix_ms) {
        return MutationEffect::NoOp;
    }
    state.hub_degraded_since_ms = Some(since_unix_ms);
    MutationEffect::Mutated
}

pub(super) fn hub_recovered(state: &mut SessionViewState) -> MutationEffect {
    if state.hub_degraded_since_ms.is_none() {
        return MutationEffect::NoOp;
    }
    state.hub_degraded_since_ms = None;
    MutationEffect::Mutated
}
