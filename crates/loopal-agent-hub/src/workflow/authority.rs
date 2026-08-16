use crate::types::{AgentExecutionRef, AgentOrigin, AgentRuntimeFacts};

use super::WorkflowOwner;

pub(crate) fn owner_for_managed_root(
    execution: &AgentExecutionRef,
    facts: &AgentRuntimeFacts,
) -> Result<WorkflowOwner, &'static str> {
    let root = &execution.address;
    if facts.origin != AgentOrigin::ManagedRoot
        || facts.parent.is_some()
        || facts.depth != 0
        || facts.root != root.agent
        || !root.is_local()
    {
        return Err("workflow control requires the authenticated managed root Agent");
    }
    let session_id = facts
        .session_id
        .clone()
        .ok_or("root Agent session is not bound")?;
    let owner = WorkflowOwner::new(session_id, root.clone());
    owner
        .is_valid()
        .then_some(owner)
        .ok_or("workflow owner is invalid")
}
