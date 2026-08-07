use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{Envelope, QualifiedAddress};
use tokio::sync::mpsc;

use super::AgentRegistry;
use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::{AgentConnectionState, ManagedAgent};

impl AgentRegistry {
    pub fn register_connection(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
    ) -> Result<(), String> {
        self.register_connection_with_parent(name, conn, None, None, None)
    }

    pub fn register_connection_with_parent(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
    ) -> Result<(), String> {
        self.register_connection_with_parent_policy(name, conn, parent, model, completion_tx, true)
    }

    pub fn register_connection_with_parent_policy(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
        notify_parent_on_completion: bool,
    ) -> Result<(), String> {
        self.remove_stale_reservation(name);
        if self.reservations.contains_key(name) {
            return Err(format!("agent '{name}' registration is pending"));
        }
        self.insert_connection(
            name,
            conn,
            parent,
            model,
            completion_tx,
            notify_parent_on_completion,
        )
    }

    pub(crate) fn reserve_connection(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
    ) -> Result<(), String> {
        self.remove_stale_reservation(name);
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        if self.reservations.contains_key(name) {
            return Err(format!("agent '{name}' registration is already pending"));
        }
        self.reservations.insert(name.to_string(), conn);
        Ok(())
    }

    pub(crate) fn activate_reserved_connection(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        completion_tx: mpsc::Sender<Envelope>,
    ) -> Result<(), String> {
        let Some(reserved) = self.reservations.get(name) else {
            return Err(format!("agent '{name}' registration reservation was lost"));
        };
        if !Arc::ptr_eq(reserved, &conn) {
            return Err(format!("agent '{name}' registration changed owner"));
        }
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        self.reservations.remove(name);
        self.insert_connection(name, conn, None, None, Some(completion_tx), true)
    }

    pub(crate) fn cancel_connection_reservation(
        &mut self,
        name: &str,
        conn: &Arc<Connection<Listening>>,
    ) -> bool {
        if self
            .reservations
            .get(name)
            .is_some_and(|reserved| Arc::ptr_eq(reserved, conn))
        {
            self.reservations.remove(name);
            true
        } else {
            false
        }
    }

    pub fn unregister_connection(&mut self, name: &str) {
        self.detach_agent(name);
        self.completions.remove(name);
    }

    pub(crate) fn unregister_connection_if_current(
        &mut self,
        name: &str,
        expected: &Arc<Connection<Listening>>,
    ) -> bool {
        if !self.is_current_connection(name, expected) {
            return false;
        }
        self.unregister_connection(name);
        true
    }

    pub(crate) fn unregister_generation_if_current(
        &mut self,
        name: &str,
        expected_generation: u64,
    ) -> bool {
        if self
            .agents
            .get(name)
            .is_none_or(|agent| agent.generation != expected_generation)
        {
            return false;
        }
        self.unregister_connection(name);
        true
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
        self.remove_stale_reservation(name);
        if self.agents.contains_key(name) || self.reservations.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        self.forget_completed(name);
        let generation = self.allocate_generation();
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
        Ok(())
    }

    fn insert_connection(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
        notify_parent_on_completion: bool,
    ) -> Result<(), String> {
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        self.forget_completed(name);
        let generation = self.allocate_generation();
        let parent_generation = parent.as_ref().and_then(|parent| {
            parent
                .is_local()
                .then(|| self.agents.get(&parent.agent).map(|agent| agent.generation))
                .flatten()
        });
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
                parent_generation,
                completion_tx,
                notify_parent_on_completion,
                view,
                completion: None,
                admitted_error: false,
                generation,
            },
        );
        Ok(())
    }

    fn remove_stale_reservation(&mut self, name: &str) {
        if self
            .reservations
            .get(name)
            .is_some_and(|connection| !connection.is_connected())
        {
            self.reservations.remove(name);
        }
    }
}
