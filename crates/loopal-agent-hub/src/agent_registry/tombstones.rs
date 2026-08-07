use loopal_protocol::{AgentCompletion, QualifiedAddress};

use super::{AgentRegistry, MAX_COMPLETED_AGENTS};
use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::{CompletedAgent, ManagedAgent};

impl AgentRegistry {
    pub fn set_completed_agent_limit(&mut self, limit: usize) {
        self.completed_limit = limit.min(MAX_COMPLETED_AGENTS);
        self.trim_completed();
    }

    pub fn completed_agent_count(&self) -> usize {
        self.completed.len()
    }

    pub(crate) fn remember_completion(&mut self, name: &str, completion: AgentCompletion) {
        if let Some(agent) = self.agents.get_mut(name) {
            agent.completion = Some(completion);
            return;
        }
        if let Some(agent) = self.completed.get_mut(name) {
            agent.completion = completion;
            return;
        }

        let mut info = AgentInfo::new(name, None, None);
        let generation = self.allocate_generation();
        info.lifecycle = if completion.is_success() {
            AgentLifecycle::Finished
        } else {
            AgentLifecycle::Failed(completion.failure_detail().to_string())
        };
        self.remember_completed(
            name,
            CompletedAgent {
                info,
                parent_generation: None,
                completion,
                view: ManagedAgent::new_view_reducer(name),
                shadow: false,
                generation,
            },
        );
    }

    pub(crate) fn detach_agent(&mut self, name: &str) {
        let Some(agent) = self.agents.remove(name) else {
            return;
        };
        if agent.info.lifecycle.is_terminal() {
            let shadow = agent.state.is_shadow();
            let completion = agent
                .completion
                .unwrap_or_else(|| match &agent.info.lifecycle {
                    AgentLifecycle::Failed(error) => {
                        AgentCompletion::new("error", Some(error.clone()))
                    }
                    _ => AgentCompletion::goal(None),
                });
            self.remember_completed(
                name,
                CompletedAgent {
                    info: agent.info,
                    parent_generation: agent.parent_generation,
                    completion,
                    view: agent.view,
                    shadow,
                    generation: agent.generation,
                },
            );
        } else {
            self.remove_from_parent(name, agent.info.parent.as_ref(), agent.parent_generation);
        }
    }

    pub(crate) fn forget_completed(&mut self, name: &str) {
        self.completed_order.retain(|entry| entry != name);
        if let Some(agent) = self.completed.remove(name) {
            self.remove_from_parent(name, agent.info.parent.as_ref(), agent.parent_generation);
        }
    }

    fn remember_completed(&mut self, name: &str, agent: CompletedAgent) {
        self.completed_order.retain(|entry| entry != name);
        self.completed.insert(name.to_string(), agent);
        self.completed_order.push_back(name.to_string());
        self.trim_completed();
    }

    fn trim_completed(&mut self) {
        while self.completed.len() > self.completed_limit {
            let Some(name) = self.completed_order.pop_front() else {
                self.completed.clear();
                break;
            };
            if let Some(agent) = self.completed.remove(&name) {
                self.remove_from_parent(&name, agent.info.parent.as_ref(), agent.parent_generation);
            }
        }
    }

    fn remove_from_parent(
        &mut self,
        child: &str,
        parent: Option<&QualifiedAddress>,
        parent_generation: Option<u64>,
    ) {
        let Some(parent) = parent.filter(|address| address.is_local()) else {
            return;
        };
        if let Some(agent) = self
            .agents
            .get_mut(&parent.agent)
            .filter(|agent| Some(agent.generation) == parent_generation)
        {
            agent.info.children.retain(|name| name != child);
        }
        if let Some(agent) = self
            .completed
            .get_mut(&parent.agent)
            .filter(|agent| Some(agent.generation) == parent_generation)
        {
            agent.info.children.retain(|name| name != child);
        }
    }
}
