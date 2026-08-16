use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{Envelope, QualifiedAddress};
use tokio::sync::mpsc;

use super::AgentRegistry;
use crate::topology::AgentInfo;
use crate::types::{AgentConnectionState, AgentExecutionRef, ManagedAgent};

pub(super) struct ConnectionRegistration<'a> {
    pub(super) parent: Option<QualifiedAddress>,
    pub(super) expected_parent: Option<&'a AgentExecutionRef>,
    pub(super) model: Option<&'a str>,
    pub(super) completion_tx: Option<mpsc::Sender<Envelope>>,
    pub(super) notify_parent_on_completion: bool,
}

impl AgentRegistry {
    pub(super) fn insert_connection(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        registration: ConnectionRegistration<'_>,
    ) -> Result<AgentExecutionRef, String> {
        let ConnectionRegistration {
            parent,
            expected_parent,
            model,
            completion_tx,
            notify_parent_on_completion,
        } = registration;
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        let parent_generation = match expected_parent {
            Some(expected) => {
                if parent.as_ref() != Some(&expected.address) || !self.owns_active_lease(expected) {
                    return Err("parent Agent connection lease is stale".into());
                }
                Some(expected.connection_generation)
            }
            None => parent.as_ref().and_then(|parent| {
                parent
                    .is_local()
                    .then(|| self.agents.get(&parent.agent).map(|agent| agent.generation))
                    .flatten()
            }),
        };
        self.forget_completed(name);
        let generation = self.allocate_generation();
        let execution = AgentExecutionRef::local(name, generation);
        if let Some(parent) = &parent
            && parent_generation.is_some()
            && let Some(parent_agent) = self.agents.get_mut(&parent.agent)
        {
            parent_agent.info.children.push(name.to_string());
        }
        let view = ManagedAgent::new_view_reducer(name);
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Connected(conn),
                info: AgentInfo::new(name, parent, model),
                runtime: None,
                parent_generation,
                completion_tx,
                notify_parent_on_completion,
                view,
                completion: None,
                admitted_error: false,
                generation,
            },
        );
        Ok(execution)
    }

    pub(super) fn remove_stale_reservation(&mut self, name: &str) {
        if self
            .reservations
            .get(name)
            .is_some_and(|connection| !connection.is_connected())
        {
            self.reservations.remove(name);
        }
    }
}
