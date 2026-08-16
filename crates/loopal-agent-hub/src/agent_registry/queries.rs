use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentCompletion, Envelope, WorkflowPermissionCausation};
use loopal_view_state::ViewStateReducer;
use tokio::sync::Mutex;

use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::{AgentConnectionState, AgentExecutionRef, AgentRuntimeFacts};

use super::AgentRegistry;

impl AgentRegistry {
    pub(crate) fn workflow_execution(
        &self,
        causation: &WorkflowPermissionCausation,
    ) -> Option<AgentExecutionRef> {
        self.agents.iter().find_map(|(name, agent)| {
            (agent
                .runtime
                .as_ref()?
                .workflow_permission_causation
                .as_ref()
                == Some(causation))
            .then(|| AgentExecutionRef::local(name.clone(), agent.generation))
        })
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn managed_agent_count(&self) -> usize {
        self.agents
            .values()
            .filter(|agent| !agent.state.is_shadow())
            .count()
    }

    pub fn sub_agent_count(&self) -> usize {
        self.agents
            .values()
            .filter(|a| a.info.parent.is_some())
            .count()
    }

    pub fn get_agent_connection(&self, name: &str) -> Option<Arc<Connection<Listening>>> {
        self.agents.get(name).and_then(|a| a.state.connection())
    }

    pub(crate) fn is_current_connection(
        &self,
        name: &str,
        expected: &Arc<Connection<Listening>>,
    ) -> bool {
        self.get_agent_connection(name)
            .is_some_and(|current| Arc::ptr_eq(&current, expected))
    }

    pub(crate) fn execution_for_connection(
        &self,
        name: &str,
        expected: &Arc<Connection<Listening>>,
    ) -> Option<AgentExecutionRef> {
        self.is_current_connection(name, expected)
            .then(|| self.current_execution(name))
            .flatten()
    }

    pub(crate) fn set_runtime_facts(
        &mut self,
        execution: &AgentExecutionRef,
        facts: AgentRuntimeFacts,
    ) -> bool {
        if !self.owns_lease(execution) {
            return false;
        }
        self.agents
            .get_mut(&execution.address.agent)
            .expect("owned execution must have an agent")
            .runtime = Some(facts);
        true
    }

    pub(crate) fn runtime_facts(
        &self,
        execution: &AgentExecutionRef,
    ) -> Option<&AgentRuntimeFacts> {
        self.owns_lease(execution)
            .then(|| self.agents.get(&execution.address.agent)?.runtime.as_ref())
            .flatten()
    }

    pub(crate) fn completion_result_limit(&self, execution: &AgentExecutionRef) -> usize {
        self.runtime_facts(execution)
            .and_then(|facts| facts.workflow_completion_result_limit)
            .map(|limit| limit as usize)
            .unwrap_or(loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES)
    }

    /// Per-agent ViewState reducer handle. Used by the hub event router
    /// to apply incoming events and by `view/snapshot` to read state.
    pub fn agent_view(&self, name: &str) -> Option<Arc<Mutex<ViewStateReducer>>> {
        self.agents
            .get(name)
            .map(|agent| agent.view.clone())
            .or_else(|| self.completed.get(name).map(|agent| agent.view.clone()))
    }

    pub fn all_agent_connections(&self) -> Vec<(String, Arc<Connection<Listening>>)> {
        self.agents
            .iter()
            .filter_map(|(n, a)| a.state.connection().map(|c| (n.clone(), c)))
            .collect()
    }

    pub fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.agents
            .iter()
            .map(|(n, a)| {
                let l = match &a.state {
                    AgentConnectionState::Local(_) => "local",
                    AgentConnectionState::Connected(_) => "connected",
                    AgentConnectionState::Shadow => "shadow",
                };
                (n.clone(), l)
            })
            .collect()
    }

    pub fn route_target(&self, envelope: &Envelope) -> Result<Arc<Connection<Listening>>, String> {
        self.get_agent_connection(&envelope.target.agent)
            .ok_or_else(|| format!("no agent: '{}'", envelope.target))
    }

    pub fn agent_info(&self, name: &str) -> Option<&AgentInfo> {
        self.agents
            .get(name)
            .map(|agent| &agent.info)
            .or_else(|| self.completed.get(name).map(|agent| &agent.info))
    }

    pub fn completion_output(&self, name: &str) -> Option<&str> {
        self.completion(name).map(AgentCompletion::output)
    }

    pub fn completion(&self, name: &str) -> Option<&AgentCompletion> {
        self.agents
            .get(name)
            .and_then(|agent| agent.completion.as_ref())
            .or_else(|| self.completed.get(name).map(|agent| &agent.completion))
    }

    pub fn set_lifecycle(&mut self, name: &str, lifecycle: AgentLifecycle) {
        if let Some(a) = self.agents.get_mut(name) {
            if a.completion.is_some() {
                return;
            }
            a.info.lifecycle = lifecycle;
        } else if self.completed.contains_key(name) {
            // Every completed entry has an authoritative typed completion.
            let _ = lifecycle;
            tracing::debug!(agent = %name, "ignoring lifecycle event for completed generation");
        }
    }

    pub(crate) fn set_completion_lifecycle(&mut self, name: &str, lifecycle: AgentLifecycle) {
        if let Some(a) = self.agents.get_mut(name) {
            a.info.lifecycle = lifecycle;
        } else if let Some(a) = self.completed.get_mut(name) {
            a.info.lifecycle = lifecycle;
        }
    }
}
