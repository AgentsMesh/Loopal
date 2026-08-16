use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, UiCapabilities};
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

async fn wait_pending(hub: &Arc<Mutex<Hub>>, id: &str) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !hub
            .lock()
            .await
            .pending_plan_approvals
            .contains_key(&("main".into(), id.into()))
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("plan request should become pending");
}

#[tokio::test]
async fn dropped_request_sends_cancel_and_removes_hub_pending_state() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let _ui = UiSession::connect(hub.clone(), "desktop", plan_capability()).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");
    let request = request_plan(agent, "cancel-request");
    wait_pending(&hub, "cancel-request").await;

    request.abort();
    let _ = request.await;
    tokio::time::timeout(Duration::from_secs(1), async {
        while hub
            .lock()
            .await
            .pending_plan_approvals
            .contains_key(&("main".into(), "cancel-request".into()))
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("$/cancelRequest should remove Hub pending state");
}

#[tokio::test]
async fn plan_timeout_returns_typed_cancellation() {
    let (hub, raw_rx) = make_hub();
    hub.lock()
        .await
        .set_pending_interaction_timeout(Duration::from_millis(30));
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let _ui = UiSession::connect(hub.clone(), "desktop", plan_capability()).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");

    let response = tokio::time::timeout(
        Duration::from_secs(1),
        agent.send_request(
            methods::AGENT_PLAN_APPROVAL.name,
            json!({
                "request_id": "timeout",
                "plan_content": "# Plan",
                "plan_path": "/tmp/plan.md",
            }),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response["decision"], "cancelled");
    assert_eq!(response["reason"], "timed_out");
    assert!(hub.lock().await.pending_plan_approvals.is_empty());
}

#[tokio::test]
async fn duplicate_id_rejects_new_rpc_and_delayed_response_resolves_old_rpc() {
    let (hub, raw_rx) = make_hub();
    hub.lock()
        .await
        .set_pending_interaction_timeout(Duration::from_millis(300));
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", plan_capability()).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");
    let old = request_plan(agent.clone(), "duplicate");
    wait_pending(&hub, "duplicate").await;
    let interaction_id = crate::plan_interaction_id(&hub, "main", "duplicate").await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let current = request_plan(agent, "duplicate");

    let current_response = tokio::time::timeout(Duration::from_secs(1), current)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(current_response["reason"], "superseded");
    assert!(
        !old.is_finished(),
        "the original request must remain pending"
    );
    assert!(
        hub.lock()
            .await
            .pending_plan_approvals
            .contains_key(&("main".into(), "duplicate".into()))
    );

    ui.client
        .respond_plan_approval("main", &interaction_id, "approve", None)
        .await;
    assert_eq!(old.await.unwrap().unwrap()["decision"], "approve");
}
