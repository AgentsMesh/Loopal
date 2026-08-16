use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::types::AgentOrigin;

pub async fn bind_managed_root_session_id(
    hub: &Arc<Mutex<Hub>>,
    connection: &Arc<Connection<Listening>>,
    session_id: &str,
) -> Result<(), String> {
    if session_id.is_empty() {
        return Err("root session id cannot be empty".into());
    }
    let mut locked = hub.lock().await;
    let execution = locked
        .registry
        .execution_for_connection(loopal_protocol::ROOT_AGENT_NAME, connection)
        .ok_or_else(|| "root Agent connection lease is stale".to_string())?;
    let mut facts = locked
        .registry
        .runtime_facts(&execution)
        .cloned()
        .ok_or_else(|| "root Agent runtime facts are unavailable".to_string())?;
    if facts.origin != AgentOrigin::ManagedRoot || facts.parent.is_some() || facts.depth != 0 {
        return Err("root session id requires a managed root Agent".into());
    }
    match facts.session_id.as_deref() {
        Some(current) if current != session_id => {
            return Err("root Agent session id is already bound".into());
        }
        Some(_) => return Ok(()),
        None => facts.session_id = Some(session_id.to_string()),
    }
    if !locked.registry.set_runtime_facts(&execution, facts) {
        return Err("root Agent lease changed before session binding".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use loopal_ipc::Connection;
    use loopal_protocol::ROOT_AGENT_NAME;
    use tokio::sync::mpsc;

    use super::*;
    use crate::types::{AgentRuntimeFacts, SpawnAuthority};

    fn connection() -> Arc<Connection<Listening>> {
        let (_peer, transport) = loopal_ipc::duplex_pair();
        Connection::new(transport).into_listening().0
    }

    async fn fixture() -> (Arc<Mutex<Hub>>, Arc<Connection<Listening>>) {
        let root = tempfile::tempdir().unwrap().keep();
        let (events, _event_rx) = mpsc::channel(8);
        let mut hub = Hub::with_cwd(events, root.clone());
        let conn = connection();
        let execution = hub
            .registry
            .register_connection_with_parent_execution(
                ROOT_AGENT_NAME,
                conn.clone(),
                None,
                None,
                None,
            )
            .unwrap();
        assert!(hub.registry.set_runtime_facts(
            &execution,
            AgentRuntimeFacts::root(root, SpawnAuthority::default()),
        ));
        (Arc::new(Mutex::new(hub)), conn)
    }

    #[tokio::test]
    async fn binds_once_to_exact_managed_root_lease() {
        let (hub, conn) = fixture().await;
        bind_managed_root_session_id(&hub, &conn, "session-1")
            .await
            .unwrap();
        bind_managed_root_session_id(&hub, &conn, "session-1")
            .await
            .unwrap();
        let locked = hub.lock().await;
        let execution = locked
            .registry
            .execution_for_connection(ROOT_AGENT_NAME, &conn)
            .unwrap();
        assert_eq!(
            locked
                .registry
                .runtime_facts(&execution)
                .unwrap()
                .session_id
                .as_deref(),
            Some("session-1")
        );
    }

    #[tokio::test]
    async fn rejects_empty_conflicting_and_stale_bindings() {
        let (hub, conn) = fixture().await;
        assert!(bind_managed_root_session_id(&hub, &conn, "").await.is_err());
        bind_managed_root_session_id(&hub, &conn, "session-1")
            .await
            .unwrap();
        assert!(
            bind_managed_root_session_id(&hub, &conn, "session-2")
                .await
                .is_err()
        );
        hub.lock()
            .await
            .registry
            .unregister_connection(ROOT_AGENT_NAME);
        assert!(
            bind_managed_root_session_id(&hub, &conn, "session-1")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_non_root_connection() {
        let (events, _event_rx) = mpsc::channel(8);
        let mut hub = Hub::new(events);
        let conn = connection();
        hub.registry
            .register_connection("worker", conn.clone())
            .unwrap();
        let hub = Arc::new(Mutex::new(hub));
        assert!(
            bind_managed_root_session_id(&hub, &conn, "session-1")
                .await
                .is_err()
        );
    }
}
