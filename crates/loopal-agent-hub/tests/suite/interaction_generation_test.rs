use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, UiCapabilities};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel(32);
    (crate::permission_support::hub_with_noop_audit(tx), rx)
}

#[tokio::test]
async fn stale_response_and_old_timer_cannot_resolve_reused_logical_id() {
    let (hub, raw_rx) = make_hub();
    hub.lock()
        .await
        .set_pending_interaction_timeout(Duration::from_millis(300));
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");
    let first = request_permission(agent.clone());
    let old = crate::permission_interaction(&hub, "main", "reused").await;
    ui.client
        .respond_permission("main", &old.id, Some(old.digest), true)
        .await;
    assert_eq!(first.await.unwrap().unwrap()["allow"], true);

    hub.lock()
        .await
        .set_pending_interaction_timeout(Duration::from_secs(2));
    let second = request_permission(agent);
    let new = crate::permission_interaction(&hub, "main", "reused").await;
    assert_ne!(old.id, new.id);
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(
        crate::permission_interaction_id(&hub, "main", "reused").await,
        new.id
    );

    let stale = ui
        .client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({"agent_name": "main", "tool_call_id": old.id, "allow": false}),
        )
        .await
        .unwrap();
    assert_eq!(stale["resolved"], false);
    assert!(
        hub.lock()
            .await
            .pending_permissions
            .contains_key(&("main".into(), "reused".into()))
    );
    ui.client
        .respond_permission("main", &new.id, Some(new.digest), true)
        .await;
    assert_eq!(second.await.unwrap().unwrap()["allow"], true);
}

#[tokio::test]
async fn question_body_id_mismatch_leaves_pending_interaction_intact() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");
    let request = tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_QUESTION.name,
                json!({"question_id": "logical", "questions": []}),
            )
            .await
    });
    let token = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(token) = hub
                .lock()
                .await
                .pending_questions
                .get(&("main".into(), "logical".into()))
                .map(|info| info.interaction_id.clone())
            {
                break token;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mismatch = ui.client.connection().send_request(
        methods::HUB_QUESTION_RESPONSE.name,
        json!({
            "agent_name": "main", "question_id": token,
            "response": {"kind": "answered", "question_id": "wrong-token", "answers": ["answer"]}
        }),
    ).await;
    assert!(mismatch.is_err());
    assert!(
        hub.lock()
            .await
            .pending_questions
            .contains_key(&("main".into(), "logical".into()))
    );
    ui.client
        .respond_question("main", &token, vec!["answer".into()])
        .await;
    assert_eq!(request.await.unwrap().unwrap()["question_id"], "logical");
}

fn request_permission(
    agent: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request("reused", "Bash", json!({})),
            )
            .await
    })
}
