//! Tests for hub/* IPC request dispatch.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::dispatch::dispatch_hub_request;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> Arc<Mutex<AgentHub>> {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    Arc::new(Mutex::new(AgentHub::new(tx)))
}

#[tokio::test]
async fn dispatch_topology_returns_agents() {
    let hub = make_hub();
    let result = dispatch_hub_request(&hub, "hub/topology", json!({}), "any".into())
        .await
        .unwrap();
    assert!(result["agents"].is_array());
}

#[tokio::test]
async fn dispatch_agent_info_not_found() {
    let hub = make_hub();
    let result = dispatch_hub_request(
        &hub, "hub/agent_info", json!({"name": "ghost"}), "any".into(),
    ).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn dispatch_list_agents_empty() {
    let hub = make_hub();
    let result = dispatch_hub_request(&hub, "hub/list_agents", json!({}), "any".into())
        .await
        .unwrap();
    assert!(result["agents"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn dispatch_unknown_method_returns_error() {
    let hub = make_hub();
    let result = dispatch_hub_request(&hub, "hub/unknown", json!({}), "any".into()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn dispatch_route_without_target_fails() {
    let hub = make_hub();
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000000",
        "source": {"Agent": "sender"},
        "target": "nonexistent",
        "content": {"text": "hi", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = dispatch_hub_request(&hub, "hub/route", envelope, "sender".into()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no agent registered"));
}
