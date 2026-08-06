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

#[tokio::test]
async fn each_interaction_kind_allows_only_one_active_request_per_agent() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");

    let permission = tokio::spawn({
        let agent = agent.clone();
        async move {
            agent
                .send_request(
                    methods::AGENT_PERMISSION.name,
                    json!({
                        "tool_call_id": "permission-a", "tool_name": "Bash", "tool_input": {}
                    }),
                )
                .await
        }
    });
    let permission_token = crate::permission_interaction_id(&hub, "main", "permission-a").await;
    let second_permission = agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({
                "tool_call_id": "permission-b", "tool_name": "Read", "tool_input": {}
            }),
        )
        .await
        .unwrap();
    assert_eq!(second_permission["allow"], false);

    let question = tokio::spawn({
        let agent = agent.clone();
        async move {
            agent
                .send_request(
                    methods::AGENT_QUESTION.name,
                    json!({"question_id": "question-a", "questions": []}),
                )
                .await
        }
    });
    let question_token = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(token) = hub
                .lock()
                .await
                .pending_questions
                .get(&("main".into(), "question-a".into()))
                .map(|info| info.interaction_id.clone())
            {
                break token;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let second_question = agent
        .send_request(
            methods::AGENT_QUESTION.name,
            json!({"question_id": "question-b", "questions": []}),
        )
        .await
        .unwrap();
    assert_eq!(second_question["kind"], "cancelled");

    let plan = request_plan(agent.clone(), "plan-a");
    let plan_token = crate::plan_interaction_id(&hub, "main", "plan-a").await;
    let second_plan = request_plan(agent, "plan-b").await.unwrap().unwrap();
    assert_eq!(second_plan["decision"], "cancelled");
    assert_eq!(second_plan["reason"], "superseded");

    {
        let h = hub.lock().await;
        assert_eq!(h.pending_permissions.len(), 1);
        assert_eq!(h.pending_questions.len(), 1);
        assert_eq!(h.pending_plan_approvals.len(), 1);
    }
    ui.client
        .respond_permission("main", &permission_token, true)
        .await;
    ui.client
        .respond_question("main", &question_token, vec!["answer".into()])
        .await;
    ui.client
        .respond_plan_approval("main", &plan_token, "approve", None)
        .await;
    assert_eq!(permission.await.unwrap().unwrap()["allow"], true);
    assert_eq!(
        question.await.unwrap().unwrap()["question_id"],
        "question-a"
    );
    assert_eq!(plan.await.unwrap().unwrap()["decision"], "approve");
}

fn request_plan(
    conn: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    id: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        conn.send_request(
            methods::AGENT_PLAN_APPROVAL.name,
            json!({
                "request_id": id, "plan_content": "# Plan", "plan_path": "/tmp/plan.md"
            }),
        )
        .await
    })
}
