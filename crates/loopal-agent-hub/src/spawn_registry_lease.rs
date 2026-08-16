use super::SpawnRegistry;
use crate::types::AgentExecutionRef;

impl SpawnRegistry {
    pub(crate) fn while_current(
        &self,
        execution: &AgentExecutionRef,
        action: impl FnOnce(),
    ) -> bool {
        let entries = self.entries.read().unwrap();
        if entries
            .get(&execution.address.agent)
            .is_some_and(|entry| entry.execution == *execution)
        {
            action();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn exact_execution_runs_action_and_stale_execution_does_not() {
        let registry = SpawnRegistry::new();
        let current = AgentExecutionRef::local("agent", 2);
        assert!(registry.register_exact(current.clone(), ".".into(), None));
        let calls = AtomicUsize::new(0);

        assert!(registry.while_current(&current, || {
            calls.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(
            !registry.while_current(&AgentExecutionRef::local("agent", 1), || {
                calls.fetch_add(1, Ordering::SeqCst);
            })
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
