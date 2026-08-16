mod activation;
mod activation_finish;
mod cleanup;
mod interrupt;
mod prepare_abort;
mod teardown;

pub(super) use activation::activate;
#[cfg(test)]
pub(super) use activation_finish::finish_activation_for_test;
pub(super) use cleanup::shutdown;
#[cfg(test)]
pub(super) use cleanup::shutdown_supervisor_for_test;
pub(super) use interrupt::interrupt;
pub(super) use prepare_abort::abort_prepare;
pub(super) use teardown::finish_exact;

use crate::types::AgentExecutionRef;

fn exact_mut<'a>(
    owners: &'a mut super::AttemptOwners,
    execution: &AgentExecutionRef,
) -> Option<&'a mut super::AttemptOwner> {
    let attempt = owners.by_execution.get(execution)?.clone();
    let owner = owners.by_attempt.get_mut(&attempt)?;
    (owner.execution == *execution).then_some(owner)
}
