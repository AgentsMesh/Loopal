use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{Envelope, QualifiedAddress};
use tokio::sync::mpsc;

use super::AgentRegistry;
use super::insertion::ConnectionRegistration;
use crate::types::AgentExecutionRef;

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
        self.register_connection_with_parent_execution(name, conn, parent, model, completion_tx)
            .map(|_| ())
    }

    pub(crate) fn register_connection_with_parent_execution(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
    ) -> Result<AgentExecutionRef, String> {
        self.register_connection_with_parent_policy_execution(
            name,
            conn,
            parent,
            model,
            completion_tx,
            true,
        )
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
        self.register_connection_with_parent_policy_execution(
            name,
            conn,
            parent,
            model,
            completion_tx,
            notify_parent_on_completion,
        )
        .map(|_| ())
    }

    pub(crate) fn register_connection_with_parent_policy_execution(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
        notify_parent_on_completion: bool,
    ) -> Result<AgentExecutionRef, String> {
        self.remove_stale_reservation(name);
        if self.reservations.contains_key(name) {
            return Err(format!("agent '{name}' registration is pending"));
        }
        self.insert_connection(
            name,
            conn,
            ConnectionRegistration {
                parent,
                expected_parent: None,
                model,
                completion_tx,
                notify_parent_on_completion,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_connection_with_exact_parent_execution(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        expected_parent: Option<&AgentExecutionRef>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
        notify_parent_on_completion: bool,
    ) -> Result<AgentExecutionRef, String> {
        self.remove_stale_reservation(name);
        if self.reservations.contains_key(name) {
            return Err(format!("agent '{name}' registration is pending"));
        }
        self.insert_connection(
            name,
            conn,
            ConnectionRegistration {
                parent,
                expected_parent,
                model,
                completion_tx,
                notify_parent_on_completion,
            },
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

    pub(crate) fn activate_reserved_connection_with_execution(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        completion_tx: mpsc::Sender<Envelope>,
    ) -> Result<AgentExecutionRef, String> {
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
        self.insert_connection(
            name,
            conn,
            ConnectionRegistration {
                parent: None,
                expected_parent: None,
                model: None,
                completion_tx: Some(completion_tx),
                notify_parent_on_completion: true,
            },
        )
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
}
