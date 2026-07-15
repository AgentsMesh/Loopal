use loopal_protocol::QualifiedAddress;

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

    pub(crate) fn remember_output(&mut self, name: &str, output: String) {
        if let Some(agent) = self.agents.get_mut(name) {
            agent.output = Some(output);
            return;
        }
        if let Some(agent) = self.completed.get_mut(name) {
            agent.output = output;
            return;
        }

        let mut info = AgentInfo::new(name, None, None);
        info.lifecycle = AgentLifecycle::Finished;
        self.remember_completed(
            name,
            CompletedAgent {
                info,
                output,
                view: ManagedAgent::new_view_reducer(name),
                shadow: false,
            },
        );
    }

    pub(crate) fn detach_agent(&mut self, name: &str) {
        let Some(agent) = self.agents.remove(name) else {
            return;
        };
        if agent.info.lifecycle.is_terminal() {
            let shadow = agent.state.is_shadow();
            let output = agent.output.unwrap_or_else(|| "(no output)".into());
            self.remember_completed(
                name,
                CompletedAgent {
                    info: agent.info,
                    output,
                    view: agent.view,
                    shadow,
                },
            );
        } else {
            self.remove_from_parent(name, agent.info.parent.as_ref());
        }
    }

    pub(crate) fn forget_completed(&mut self, name: &str) {
        self.completed_order.retain(|entry| entry != name);
        if let Some(agent) = self.completed.remove(name) {
            self.remove_from_parent(name, agent.info.parent.as_ref());
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
                self.remove_from_parent(&name, agent.info.parent.as_ref());
            }
        }
    }

    fn remove_from_parent(&mut self, child: &str, parent: Option<&QualifiedAddress>) {
        let Some(parent) = parent.filter(|address| address.is_local()) else {
            return;
        };
        if let Some(agent) = self.agents.get_mut(&parent.agent) {
            agent.info.children.retain(|name| name != child);
        }
        if let Some(agent) = self.completed.get_mut(&parent.agent) {
            agent.info.children.retain(|name| name != child);
        }
    }
}
