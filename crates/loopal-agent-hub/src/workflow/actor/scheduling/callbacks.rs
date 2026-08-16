mod abort;
mod activation;
mod outcome;
mod preparation;
mod preparation_activation;

pub(in crate::workflow::actor) use abort::{
    contain_late_preparation, late_preparation_shutdown, preparation_abort_settled,
    preparation_aborted,
};
pub(in crate::workflow::actor) use activation::{activated, spawn_outcome_waiter};
pub(in crate::workflow::actor) use outcome::{finished, outcome_lost};
pub(in crate::workflow::actor) use preparation::{
    preparation_delivery_finished, preparation_timed_out, prepared,
};

use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{ActiveAttemptPhase, AttemptKey};

use super::super::WorkflowCoordinator;

pub(super) fn matches_active(
    coordinator: &WorkflowCoordinator,
    owner: &crate::workflow::WorkflowOwner,
    key: &AttemptKey,
    execution: &AgentExecutionRef,
    phase: Option<ActiveAttemptPhase>,
) -> bool {
    coordinator
        .active
        .get(&key.attempt_id)
        .is_some_and(|active| {
            active.owner == *owner
                && active.key == *key
                && active.execution == *execution
                && phase.is_none_or(|phase| active.phase == phase)
        })
}

pub(super) fn valid_lease(execution: &AgentExecutionRef) -> bool {
    execution.connection_generation != 0
        && execution.address.is_local()
        && !execution.address.agent.is_empty()
        && execution.address.agent.len() <= 128
        && !execution.address.agent.contains('/')
}

pub(super) fn unique_lease(
    coordinator: &WorkflowCoordinator,
    execution: &AgentExecutionRef,
) -> bool {
    valid_lease(execution)
        && !coordinator
            .active
            .values()
            .any(|active| active.execution == *execution)
}
