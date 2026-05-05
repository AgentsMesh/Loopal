use loopal_protocol::{AgentStatus, ObservableAgentState};
use loopal_view_state::AgentConversation;

use super::App;
use crate::view_client::ViewClient;

impl App {
    pub fn view_client_for(&self, agent: &str) -> ViewClient {
        self.view_clients
            .get(agent)
            .or_else(|| self.view_clients.get("main"))
            .expect("at least 'main' ViewClient is always present")
            .clone()
    }

    pub fn active_view_client(&self) -> ViewClient {
        let active = self.session.lock().active_view.clone();
        self.view_client_for(&active)
    }

    pub fn observable_for(&self, agent: &str) -> ObservableAgentState {
        self.view_clients
            .get(agent)
            .map(|vc| vc.state().state().agent.observable.clone())
            .unwrap_or_default()
    }

    pub fn is_agent_live(&self, agent: &str) -> bool {
        let status = self.observable_for(agent).status;
        !matches!(status, AgentStatus::Finished | AgentStatus::Error)
    }

    pub fn is_active_agent_idle(&self) -> bool {
        let active = self.session.lock().active_view.clone();
        let status = self.observable_for(&active).status;
        matches!(
            status,
            AgentStatus::WaitingForInput | AgentStatus::Finished | AgentStatus::Error
        )
    }

    pub fn with_active_conversation<R>(&self, f: impl FnOnce(&AgentConversation) -> R) -> R {
        let vc = self.active_view_client();
        let guard = vc.state();
        f(guard.conversation())
    }

    pub fn with_active_conversation_mut<R>(
        &self,
        f: impl FnOnce(&mut AgentConversation) -> R,
    ) -> R {
        let vc = self.active_view_client();
        vc.with_conversation_mut(f)
    }

    pub fn snapshot_active_conversation(&self) -> AgentConversation {
        self.with_active_conversation(|c| c.clone())
    }

    pub fn snapshot_conversation(&self, name: &str) -> AgentConversation {
        let vc = self.view_client_for(name);
        let guard = vc.state();
        guard.conversation().clone()
    }
}
