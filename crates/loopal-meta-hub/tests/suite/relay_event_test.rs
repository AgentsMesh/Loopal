//! Tests: permission relay, event aggregation, hub disconnect.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

#[tokio::test]
async fn permission_relay_via_metahub_ui() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_b, _) = make_hub();
    let hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_b_conn, "hub-b".into()));
        hub_b.lock().await.uplink = Some(ul);
    }

    let (ui_client_t, ui_server_t) = loopal_ipc::duplex_pair();
    let ui_client = Arc::new(Connection::new(ui_client_t));
    let ui_server = Arc::new(Connection::new(ui_server_t));
    let ui_rx = ui_client.start();
    let _ui_srv_rx = ui_server.start();
    {
        meta_hub
            .lock()
            .await
            .ui
            .register_client("meta-ui", ui_server);
    }

    let ucc = ui_client.clone();
    tokio::spawn(async move {
        let mut rx = ui_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = ucc.respond(id, json!({"allow": true})).await;
            }
        }
    });

    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::AGENT_PERMISSION.name,
        json!({"tool": "bash", "agent_name": "test"}),
        "hub-b".into(),
    )
    .await;
    assert_eq!(result.unwrap()["allow"].as_bool(), Some(true));
}

#[tokio::test]
async fn permission_denied_without_metahub_ui() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::AGENT_PERMISSION.name,
        json!({"tool": "bash", "agent_name": "lonely"}),
        "hub-x".into(),
    )
    .await;
    assert_eq!(result.unwrap()["allow"].as_bool(), Some(false));
}

#[tokio::test]
async fn event_aggregation_prefixes_hub_name() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_b, _) = make_hub();
    let _conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut event_rx = meta_hub.lock().await.aggregator.subscribe();
    let mut event = AgentEvent::named("test-agent", loopal_protocol::AgentEventPayload::Started);
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-b");
    let _ = meta_hub.lock().await.aggregator.broadcaster().send(event);

    let received = tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await;
    assert_eq!(
        received.unwrap().unwrap().agent_name.as_deref(),
        Some("hub-b/test-agent")
    );
}

#[tokio::test]
async fn hub_disconnect_cleans_up_registry() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (_, meta_transport) = loopal_ipc::duplex_pair();
    let meta_conn = Arc::new(Connection::new(meta_transport));
    let _rx = meta_conn.start();
    {
        let mut mh = meta_hub.lock().await;
        mh.registry
            .register("dying-hub", meta_conn, vec![])
            .unwrap();
        mh.router.cache_insert("agent-on-dying", "dying-hub");
        mh.remove_hub("dying-hub");
        assert_eq!(mh.registry.len(), 0);
        assert!(mh.router.cache_lookup("agent-on-dying").is_none());
    }
}

/// Hub with local UI clients does NOT relay via uplink (regression).
#[tokio::test]
async fn local_ui_skips_uplink_relay() {
    let (hub, _) = make_hub();

    // Set uplink
    {
        let (t, _) = loopal_ipc::duplex_pair();
        let c = Arc::new(Connection::new(t));
        let _rx = c.start();
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(c, "hub-x".into()));
        hub.lock().await.uplink = Some(ul);
    }

    // Register local UI client that auto-approves
    let (ui_client_t, ui_server_t) = loopal_ipc::duplex_pair();
    let ui_client = Arc::new(Connection::new(ui_client_t));
    let ui_server = Arc::new(Connection::new(ui_server_t));
    let ui_rx = ui_client.start();
    let _ui_srv_rx = ui_server.start();
    hub.lock().await.ui.register_client("local-ui", ui_server);

    let ucc = ui_client.clone();
    tokio::spawn(async move {
        let mut rx = ui_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = ucc
                    .respond(id, json!({"allow": true, "source": "local"}))
                    .await;
            }
        }
    });

    // Agent sends permission → should go to local UI, not uplink
    let (ac, agent_rx) = loopal_agent_hub::hub_server::connect_local(hub.clone(), "agent");
    tokio::spawn(async move {
        let mut rx = agent_rx;
        while rx.recv().await.is_some() {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let resp = ac
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool": "bash", "command": "ls"}),
        )
        .await
        .unwrap();
    assert_eq!(resp["allow"].as_bool(), Some(true));
    // If it went through uplink instead of local UI, the response would differ
    assert_eq!(resp["source"].as_str(), Some("local"));
}

/// Events from multiple Hubs coexist in same broadcast.
#[tokio::test]
async fn multi_hub_events_in_single_broadcast() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let _conn_a = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _conn_b = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut event_rx = meta_hub.lock().await.aggregator.subscribe();
    let broadcaster = meta_hub.lock().await.aggregator.broadcaster();

    // Inject events from two different hubs
    let mut ev_a = AgentEvent::named("worker-1", loopal_protocol::AgentEventPayload::Started);
    loopal_meta_hub::aggregator::prefix_agent_name(&mut ev_a, "hub-a");
    let _ = broadcaster.send(ev_a);

    let mut ev_b = AgentEvent::named("worker-2", loopal_protocol::AgentEventPayload::Started);
    loopal_meta_hub::aggregator::prefix_agent_name(&mut ev_b, "hub-b");
    let _ = broadcaster.send(ev_b);

    let e1 = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let e2 = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .unwrap()
        .unwrap();

    let names: Vec<_> = [e1, e2]
        .iter()
        .filter_map(|e| e.agent_name.clone())
        .collect();
    assert!(names.contains(&"hub-a/worker-1".to_string()));
    assert!(names.contains(&"hub-b/worker-2".to_string()));
}
