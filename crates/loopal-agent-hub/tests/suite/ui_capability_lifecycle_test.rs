use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, UiCapabilities};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel(32);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

fn plan_capability() -> UiCapabilities {
    UiCapabilities {
        plan_approval: true,
        ..UiCapabilities::NONE
    }
}

async fn wait_plan_pending(hub: &Arc<Mutex<Hub>>, id: &str) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if hub
                .lock()
                .await
                .pending_plan_approvals
                .contains_key(&("main".into(), id.into()))
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("plan request should become pending");
}

fn request_plan(
    conn: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    id: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        conn.send_request(
            methods::AGENT_PLAN_APPROVAL.name,
            json!({
                "request_id": id,
                "plan_content": "# Plan",
                "plan_path": "/tmp/plan.md",
            }),
        )
        .await
    })
}

#[tokio::test]
async fn observer_ui_does_not_enable_interactive_requests() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let _observer = UiSession::connect(hub.clone(), "observer", UiCapabilities::NONE).await;
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");

    let permission = agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            crate::permission_request("p", "Bash", json!({})),
        )
        .await
        .unwrap();
    assert_eq!(permission["allow"], false);

    let plan = request_plan(agent, "none").await.unwrap().unwrap();
    assert_eq!(plan["decision"], "cancelled");
    assert_eq!(plan["reason"], "unavailable");
    assert!(hub.lock().await.pending_plan_approvals.is_empty());
}

#[tokio::test]
async fn incapable_responder_is_denied_without_consuming_pending() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let capable = UiSession::connect(hub.clone(), "capable", plan_capability()).await;
    let observer = UiSession::connect(hub.clone(), "observer", UiCapabilities::NONE).await;
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");
    let request = request_plan(agent, "auth");
    wait_plan_pending(&hub, "auth").await;
    let interaction_id = crate::plan_interaction_id(&hub, "main", "auth").await;

    let unauthorized = observer
        .client
        .connection()
        .send_request(
            methods::HUB_PLAN_APPROVAL_RESPONSE.name,
            json!({"agent_name": "main", "request_id": interaction_id.clone(), "decision": "approve"}),
        )
        .await;
    assert!(matches!(
        unauthorized,
        Err(loopal_ipc::RpcError::Remote { .. })
    ));
    assert!(
        hub.lock()
            .await
            .pending_plan_approvals
            .contains_key(&("main".into(), "auth".into()))
    );

    capable
        .client
        .respond_plan_approval("main", &interaction_id, "approve", None)
        .await;
    assert_eq!(request.await.unwrap().unwrap()["decision"], "approve");
}

#[tokio::test]
async fn dropping_last_capable_session_cancels_pending_and_emits_resolved() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", plan_capability()).await;
    let mut events = hub.lock().await.ui.event_broadcaster().subscribe();
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");
    let request = request_plan(agent, "drop-last");
    wait_plan_pending(&hub, "drop-last").await;
    let interaction_id = crate::plan_interaction_id(&hub, "main", "drop-last").await;

    drop(ui);
    let response = tokio::time::timeout(Duration::from_secs(1), request)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(response["decision"], "cancelled");
    assert_eq!(response["reason"], "unavailable");

    let resolved = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(
                event.payload,
                AgentEventPayload::PlanApprovalResolved { .. }
            ) {
                return event;
            }
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        resolved.payload,
        AgentEventPayload::PlanApprovalResolved { id } if id == interaction_id
    ));
}

#[tokio::test]
async fn same_name_leases_are_independent_until_last_capable_owner_drops() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let old = UiSession::connect(hub.clone(), "desktop", plan_capability()).await;
    let current = UiSession::connect(hub.clone(), "desktop", plan_capability()).await;
    assert_ne!(old.lease_id, current.lease_id);
    let current_lease = current.lease_id.clone();
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");
    let request = request_plan(agent, "same-name");
    wait_plan_pending(&hub, "same-name").await;
    let interaction_id = crate::plan_interaction_id(&hub, "main", "same-name").await;

    drop(old);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(hub.lock().await.ui.is_ui_client(&current_lease));
    assert!(!request.is_finished());

    current
        .client
        .respond_plan_approval("main", &interaction_id, "approve", None)
        .await;
    assert_eq!(request.await.unwrap().unwrap()["decision"], "approve");
}
