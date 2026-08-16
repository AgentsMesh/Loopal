//! Tests: cross-hub spawn + completion delivery.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

#[tokio::test]
async fn spawn_with_target_hub_reaches_metahub() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _hub_a_event_rx) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let error = send_agent_request(
        &hub_a,
        "parent-agent",
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "remote-worker", "target_hub": "hub-b"}),
    )
    .await
    .expect_err("missing destination Hub must reject the spawn");

    assert!(error.to_string().contains("hub-b"), "got: {error}");
}

#[tokio::test]
async fn cross_hub_spawn_reaches_target_hub() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _hub_a_event_rx) = make_hub();
    let (hub_b, _hub_b_event_rx) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    hub_b.lock().await.max_agent_depth = 0;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }

    let error = send_agent_request(
        &hub_a,
        "parent-agent",
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "remote-worker", "target_hub": "hub-b"}),
    )
    .await
    .expect_err("target Hub must enforce its depth authority");

    assert!(
        error.to_string().contains("depth limit exceeded"),
        "got: {error}"
    );
    assert!(
        hub_b
            .lock()
            .await
            .registry
            .agent_info("remote-worker")
            .is_none()
    );
    assert!(
        hub_a
            .lock()
            .await
            .registry
            .agent_info("remote-worker")
            .is_none()
    );
}

#[tokio::test]
async fn completion_delivery_to_remote_parent() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _hub_a_event_rx) = make_hub();
    let (hub_b, _hub_b_event_rx) = make_hub();
    let _hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;

    let (_parent_conn, mut parent_rx) = register_mock_agent(&hub_a, "parent-agent", None).await;
    hub_a
        .lock()
        .await
        .registry
        .register_shadow(
            "child-worker",
            loopal_protocol::QualifiedAddress::local("parent-agent"),
        )
        .unwrap();
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_b_conn, "hub-b".into()));
        hub_b.lock().await.uplink = Some(ul);
    }

    let (child_client_conn, _child_client_rx) = register_remote_agent(
        &hub_b,
        "child-worker",
        loopal_protocol::QualifiedAddress::remote(["hub-a"], "parent-agent"),
    )
    .await;

    child_client_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "error", "result": "partial remote result"}),
        )
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(msg) = parent_rx.recv().await {
            let params = match &msg {
                Incoming::Request { params, .. } | Incoming::Notification { params, .. } => params,
            };
            // Completion is now a typed AgentResult source carrying the child
            // name; the body is the raw output, not a wrapped marker.
            let is_result = params
                .get("source")
                .and_then(|s| s.get("AgentResult"))
                .and_then(|r| r.get("child"))
                .and_then(|c| c.get("agent"))
                .and_then(|a| a.as_str())
                .is_some_and(|name| name == "child-worker");
            if is_result {
                return serde_json::from_value::<loopal_protocol::Envelope>(params.clone()).ok();
            }
        }
        None
    })
    .await;

    let envelope = received
        .expect("parent completion timed out")
        .expect("parent should receive remote child completion");
    assert_eq!(envelope.content.text, "partial remote result");
    let completion = envelope
        .agent_completion
        .expect("cross-hub completion metadata must survive routing");
    assert_eq!(completion.reason, "error");
    assert_eq!(completion.result.as_deref(), Some("partial remote result"));
}
