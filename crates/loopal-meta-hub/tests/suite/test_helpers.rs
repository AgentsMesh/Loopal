//! Shared test helpers for cross-hub integration tests.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{AgentLifecycle, Hub};
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, SubAgentSpawn};
use serde_json::json;

use loopal_meta_hub::MetaHub;

pub fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    let mut hub = Hub::new(tx);
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    (Arc::new(Mutex::new(hub)), rx)
}

/// Wire a Sub-Hub to MetaHub via in-process duplex (bidirectional).
pub async fn wire_hub_to_meta(
    hub_name: &str,
    hub: &Arc<Mutex<Hub>>,
    meta_hub: &Arc<Mutex<MetaHub>>,
) -> Arc<Connection<Listening>> {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_conn, hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta_conn, meta_rx) = Connection::new(meta_transport).into_listening();

    {
        let mut mh = meta_hub.lock().await;
        mh.registry
            .register(hub_name, meta_conn.clone(), vec![])
            .unwrap();
    }

    let mh = meta_hub.clone();
    let meta_name = hub_name.to_string();
    tokio::spawn(async move {
        loopal_meta_hub::io_loop::meta_hub_io_loop(mh, meta_conn, meta_rx, meta_name).await;
    });

    // Use shared reverse handler from uplink module (no code duplication)
    let reverse_hub = hub.clone();
    let reverse_conn = hub_conn.clone();
    let reverse_name = hub_name.to_string();
    tokio::spawn(async move {
        loopal_agent_hub::uplink::handle_reverse_requests(
            reverse_hub,
            reverse_conn,
            hub_rx,
            reverse_name,
        )
        .await;
    });

    // Mirror production: reverse traffic is authoritative only for the
    // currently installed authenticated uplink lease.
    hub.lock().await.uplink = Some(Arc::new(loopal_agent_hub::HubUplink::new(
        hub_conn.clone(),
        hub_name.to_string(),
    )));

    hub_conn
}

pub async fn register_mock_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    parent: Option<&str>,
) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
    assert!(
        parent.is_none(),
        "remote agents require typed fixture setup"
    );
    let (client_conn, client_rx) = loopal_agent_hub::hub_server::connect_local(hub.clone(), name);
    wait_for_agent(hub, name).await;
    auto_respond(client_conn, client_rx)
}

pub async fn send_agent_request(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, loopal_ipc::RpcError> {
    let (connection, _incoming) = loopal_agent_hub::hub_server::connect_local(hub.clone(), name);
    wait_for_agent(hub, name).await;
    connection.send_request(method, params).await
}

pub async fn register_remote_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    parent: QualifiedAddress,
) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
    assert!(
        parent.is_remote(),
        "remote fixture requires a qualified parent"
    );
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (server_conn, server_rx) = Connection::new(server_transport).into_listening();
    let (client_conn, client_rx) = Connection::new(client_transport).into_listening();

    let event_tx = {
        let mut locked = hub.lock().await;
        locked
            .registry
            .register_connection_with_parent(
                name,
                server_conn.clone(),
                Some(parent.clone()),
                None,
                None,
            )
            .expect("remote agent registration must succeed");
        locked.registry.set_lifecycle(name, AgentLifecycle::Running);
        locked.registry.event_sender()
    };
    event_tx
        .send(AgentEvent::named(
            parent.clone(),
            AgentEventPayload::SubAgentSpawned(SubAgentSpawn {
                name: name.to_string(),
                agent_id: format!("fixture-{name}"),
                parent: Some(parent),
                model: None,
                session_id: None,
            }),
        ))
        .await
        .expect("remote spawn event receiver must remain active");
    let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    loopal_agent_hub::agent_io::spawn_io_loop(
        hub.clone(),
        dispatcher,
        name,
        server_conn,
        server_rx,
    );

    (client_conn, client_rx)
}

async fn wait_for_agent(hub: &Arc<Mutex<Hub>>, name: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while hub
            .lock()
            .await
            .registry
            .get_agent_connection(name)
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("managed Agent IO must register its connection");
}

fn auto_respond(
    client_conn: Arc<Connection<Listening>>,
    client_rx: mpsc::Receiver<Incoming>,
) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
    let cc = client_conn.clone();
    let mut listen_rx = client_rx;
    let (forward_tx, forward_rx) = mpsc::channel::<Incoming>(64);
    tokio::spawn(async move {
        while let Some(msg) = listen_rx.recv().await {
            if let Incoming::Request { id, .. } = &msg {
                let _ = cc.respond(*id, json!({"ok": true})).await;
            }
            let _ = forward_tx.send(msg).await;
        }
    });

    (client_conn, forward_rx)
}
