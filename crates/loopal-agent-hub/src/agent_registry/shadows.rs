use loopal_protocol::QualifiedAddress;

use super::AgentRegistry;
use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::{AgentConnectionState, AgentExecutionRef, ManagedAgent};

impl AgentRegistry {
    pub(crate) fn validate_shadow_registration(&mut self, name: &str) -> Result<(), String> {
        self.remove_stale_reservation(name);
        if self.agents.contains_key(name) || self.reservations.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        Ok(())
    }

    pub fn register_shadow(&mut self, name: &str, parent: QualifiedAddress) -> Result<(), String> {
        self.register_shadow_with_parent_policy(name, parent, true)
    }

    pub fn register_shadow_with_parent_policy(
        &mut self,
        name: &str,
        parent: QualifiedAddress,
        notify_parent_on_completion: bool,
    ) -> Result<(), String> {
        self.register_shadow_with_parent_policy_execution(name, parent, notify_parent_on_completion)
            .map(|_| ())
    }

    pub(crate) fn register_shadow_with_parent_policy_execution(
        &mut self,
        name: &str,
        parent: QualifiedAddress,
        notify_parent_on_completion: bool,
    ) -> Result<AgentExecutionRef, String> {
        self.validate_shadow_registration(name)?;
        self.forget_completed(name);
        let generation = self.allocate_generation();
        let execution = AgentExecutionRef::local(name, generation);
        let parent_for_children = parent.clone();
        let parent_generation = if parent_for_children.is_local() {
            self.agents
                .get(&parent_for_children.agent)
                .map(|parent| parent.generation)
        } else {
            None
        };
        let mut info = AgentInfo::new(name, Some(parent), None);
        info.lifecycle = AgentLifecycle::Running;
        let view = ManagedAgent::new_view_reducer(name);
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Shadow,
                info,
                runtime: None,
                parent_generation,
                completion_tx: None,
                notify_parent_on_completion,
                view,
                completion: None,
                admitted_error: false,
                generation,
            },
        );
        if parent_generation.is_some()
            && let Some(parent) = self.agents.get_mut(&parent_for_children.agent)
        {
            parent.info.children.push(name.to_string());
        }
        tracing::info!(agent = %name, parent = %parent_for_children,
            "shadow registered for remote agent");
        Ok(execution)
    }
}
