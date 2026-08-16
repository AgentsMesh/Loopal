use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{HubUplink, UiSession, start_event_loop};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_meta_hub::MetaHub;
use loopal_protocol::UiCapabilities;
use serde_json::json;
use tokio::sync::Mutex;

use crate::test_helpers::{make_hub, register_mock_agent, register_remote_agent, wire_hub_to_meta};

#[tokio::test]
async fn topology_queries_slow_hubs_concurrently_with_one_bound() {
    let meta = Arc::new(Mutex::new(MetaHub::new()));
    let (fast, _fast_event_rx) = make_hub();
    let _fast = wire_hub_to_meta("fast", &fast, &meta).await;
    let mut stalled = Vec::new();
    for name in ["slow-a", "slow-b", "slow-c"] {
        let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
        let (hub_conn, hub_rx) = Connection::new(hub_transport).into_listening();
        let (meta_conn, _meta_rx) = Connection::new(meta_transport).into_listening();
        meta.lock()
            .await
            .registry
            .register(name, meta_conn, vec![])
            .unwrap();
        stalled.push((hub_conn, hub_rx));
    }
    let started = tokio::time::Instant::now();
    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta,
        methods::META_TOPOLOGY.name,
        json!({}),
        "fast".into(),
    )
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(2_700));
    let hubs = result["hubs"].as_array().unwrap();
    assert_eq!(hubs.len(), 4);
    assert_eq!(hubs[0]["hub"], "fast");
    assert_eq!(
        hubs.iter()
            .filter(|hub| hub["topology"]["error"] == "timeout")
            .count(),
        3,
    );
    drop(stalled);
}

#[tokio::test]
async fn global_topology_removes_source_shadows_and_keeps_remote_parent_path() {
    let meta = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, hub_a_event_rx) = make_hub();
    let (hub_b, hub_b_event_rx) = make_hub();
    let _event_loop_a = start_event_loop(hub_a.clone(), hub_a_event_rx);
    let _event_loop_b = start_event_loop(hub_b.clone(), hub_b_event_rx);
    let (_main, _main_rx) = register_mock_agent(&hub_a, "main", None).await;
    hub_a
        .lock()
        .await
        .registry
        .register_shadow(
            "remote-child",
            loopal_protocol::QualifiedAddress::local("main"),
        )
        .unwrap();
    assert_eq!(hub_a.lock().await.registry.agent_count(), 2);
    assert_eq!(hub_a.lock().await.registry.managed_agent_count(), 1);
    let (_child, _child_rx) = register_remote_agent(
        &hub_b,
        "remote-child",
        loopal_protocol::QualifiedAddress::remote(["hub-a"], "main"),
    )
    .await;
    install_topology_ui("hub-a", &hub_a, &meta).await;
    install_topology_ui("hub-b", &hub_b, &meta).await;

    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta,
        methods::META_TOPOLOGY.name,
        json!({}),
        "hub-a".into(),
    )
    .await
    .unwrap();
    let hubs = result["hubs"].as_array().unwrap();
    let source = hubs.iter().find(|hub| hub["hub"] == "hub-a").unwrap();
    let target = hubs.iter().find(|hub| hub["hub"] == "hub-b").unwrap();
    let source_agents = source["topology"]["agents"].as_array().unwrap();
    assert_eq!(source_agents.len(), 1);
    assert_eq!(source_agents[0]["name"], "main");
    assert_eq!(source_agents[0]["children"], json!([]));
    let child = &target["topology"]["agents"][0];
    assert_eq!(child["name"], "remote-child");
    assert_eq!(child["parent"], "hub-a/main");
}

async fn install_topology_ui(
    hub_name: &str,
    hub: &Arc<Mutex<loopal_agent_hub::Hub>>,
    meta: &Arc<Mutex<MetaHub>>,
) {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_connection, mut hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta_connection, meta_rx) = Connection::new(meta_transport).into_listening();
    meta.lock()
        .await
        .registry
        .register(hub_name, meta_connection.clone(), vec![])
        .unwrap();
    let meta_loop = meta.clone();
    let name = hub_name.to_string();
    tokio::spawn(async move {
        loopal_meta_hub::io_loop::meta_hub_io_loop(meta_loop, meta_connection, meta_rx, name).await;
    });
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(
        hub_connection.clone(),
        hub_name.to_string(),
    )));
    let ui = UiSession::connect(hub.clone(), "topology-ui", UiCapabilities::NONE).await;
    tokio::spawn(async move {
        while let Some(Incoming::Request { id, method, params }) = hub_rx.recv().await {
            let result = ui.client.connection().send_request(&method, params).await;
            match result {
                Ok(value) => {
                    let _ = hub_connection.respond(id, value).await;
                }
                Err(error) => {
                    let _ = hub_connection
                        .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &error.to_string())
                        .await;
                }
            }
        }
    });
}
