use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel(16);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

#[tokio::test]
async fn relays_all_plan_approval_decisions() {
    for (decision, edited) in [
        ("approve", None),
        ("reject", None),
        ("approve_with_edits", Some("# Edited plan")),
    ] {
        let (hub, raw_rx) = make_hub();
        let _loop = start_event_loop(hub.clone(), raw_rx);
        let ui = UiSession::connect(hub.clone(), "desktop").await;
        let (agent, _) = hub_server::connect_local(hub.clone(), "main");
        tokio::time::sleep(Duration::from_millis(20)).await;
        let request = tokio::spawn(async move {
            agent
                .send_request(
                    methods::AGENT_PLAN_APPROVAL.name,
                    serde_json::json!({
                        "plan_content": "# Original plan", "plan_path": "/tmp/plan.md",
                    }),
                )
                .await
                .unwrap()
        });
        let mut events = ui.event_rx;
        let event = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let event = events.recv().await.unwrap();
                if matches!(event.payload, AgentEventPayload::PlanApprovalRequest { .. }) {
                    return event;
                }
            }
        })
        .await
        .unwrap();
        let (id, agent_name) = match event.payload {
            AgentEventPayload::PlanApprovalRequest {
                id,
                plan_content,
                plan_path,
            } => {
                assert_eq!(plan_content, "# Original plan");
                assert_eq!(plan_path, "/tmp/plan.md");
                (id, event.agent_name.unwrap().agent)
            }
            _ => unreachable!(),
        };
        ui.client
            .respond_plan_approval(&agent_name, &id, decision, edited)
            .await;
        let response = request.await.unwrap();
        assert_eq!(response["decision"], decision);
        if let Some(value) = edited {
            assert_eq!(response["edited_plan"], value);
        }
        assert!(hub.lock().await.pending_plan_approvals.is_empty());
    }
}

#[tokio::test]
async fn no_ui_rejects_plan_without_leaking_pending_state() {
    let (hub, _rx) = make_hub();
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");
    let response = agent
        .send_request(
            methods::AGENT_PLAN_APPROVAL.name,
            serde_json::json!({
                "plan_content": "# Plan", "plan_path": "/tmp/plan.md",
            }),
        )
        .await
        .unwrap();
    assert_eq!(response["decision"], "reject");
    assert!(hub.lock().await.pending_plan_approvals.is_empty());
}
