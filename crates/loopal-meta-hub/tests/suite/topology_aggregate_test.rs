use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_meta_hub::MetaHub;
use serde_json::json;
use tokio::sync::Mutex;

use crate::test_helpers::{make_hub, register_mock_agent, wire_hub_to_meta};

#[tokio::test]
async fn topology_queries_slow_hubs_concurrently_with_one_bound() {
    let meta = Arc::new(Mutex::new(MetaHub::new()));
    let (fast, _) = make_hub();
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
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
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
    let (_child, _child_rx) = register_mock_agent(&hub_b, "remote-child", Some("hub-a/main")).await;
    let _a = wire_hub_to_meta("hub-a", &hub_a, &meta).await;
    let _b = wire_hub_to_meta("hub-b", &hub_b, &meta).await;

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
