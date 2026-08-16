use super::AgentRegistry;
use crate::types::AgentExecutionRef;

impl AgentRegistry {
    pub fn unregister_connection(&mut self, name: &str) {
        self.detach_agent(name);
        self.completions.remove(name);
    }

    pub(crate) fn unregister_generation_if_current(
        &mut self,
        name: &str,
        expected_generation: u64,
    ) -> bool {
        self.unregister_exact(&AgentExecutionRef::local(name, expected_generation))
    }

    pub(crate) fn unregister_exact(&mut self, execution: &AgentExecutionRef) -> bool {
        if execution.address.is_remote()
            || self
                .agents
                .get(&execution.address.agent)
                .is_none_or(|agent| agent.generation != execution.connection_generation)
        {
            return false;
        }
        self.unregister_connection(&execution.address.agent);
        true
    }
}
