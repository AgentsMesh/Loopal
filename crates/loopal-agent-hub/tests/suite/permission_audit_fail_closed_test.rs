use std::sync::Arc;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, UiCapabilities};
use tokio::sync::{Mutex, mpsc};

use crate::protected_audit_support::CapturingSink;

struct Fixture {
    hub: Arc<Mutex<Hub>>,
    ui: UiSession,
    agent: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
}

async fn setup(sink: Arc<CapturingSink>) -> Fixture {
    let (events, receiver) = mpsc::channel::<AgentEvent>(32);
    let mut hub = Hub::new(events);
    hub.set_protected_audit(sink);
    let hub = Arc::new(Mutex::new(hub));
    let _event_loop = start_event_loop(hub.clone(), receiver);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _incoming) = hub_server::connect_local(hub.clone(), "main");
    Fixture { hub, ui, agent }
}

fn request(
    agent: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    id: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request(id, "Write", serde_json::json!({})),
            )
            .await
    })
}

async fn respond(fixture: &Fixture, id: &str, allow: bool, remember_session: bool) {
    let interaction = crate::permission_interaction(&fixture.hub, "main", id).await;
    fixture
        .ui
        .client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            serde_json::json!({
                "agent_name": "main",
                "tool_call_id": interaction.id,
                "permission_intent_digest": interaction.digest,
                "allow": allow,
                "remember_session": remember_session,
            }),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn ui_allow_audit_failure_denies_and_installs_no_grant() {
    let failing = Arc::new(CapturingSink::new(true));
    let fixture = setup(failing.clone()).await;
    let first = request(fixture.agent.clone(), "first");
    respond(&fixture, "first", true, true).await;
    assert_eq!(first.await.unwrap().unwrap()["allow"], false);

    let records = failing.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision.as_deref(), Some("allow"));
    assert_eq!(records[0].decision_source.as_deref(), Some("ui"));

    fixture
        .hub
        .lock()
        .await
        .set_protected_audit(Arc::new(CapturingSink::new(false)));
    let retry = request(fixture.agent.clone(), "retry");
    respond(&fixture, "retry", false, false).await;
    assert_eq!(retry.await.unwrap().unwrap()["allow"], false);
}

#[tokio::test]
async fn authorized_ui_deny_is_audited() {
    let sink = Arc::new(CapturingSink::new(false));
    let fixture = setup(sink.clone()).await;
    let pending = request(fixture.agent.clone(), "denied");
    respond(&fixture, "denied", false, false).await;
    assert_eq!(pending.await.unwrap().unwrap()["allow"], false);

    let records = sink.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision.as_deref(), Some("deny"));
    assert_eq!(records[0].decision_source.as_deref(), Some("ui"));
}

#[tokio::test]
async fn remembered_grant_audit_failure_denies_reuse() {
    let healthy = Arc::new(CapturingSink::new(false));
    let fixture = setup(healthy).await;
    let first = request(fixture.agent.clone(), "grant");
    respond(&fixture, "grant", true, true).await;
    assert_eq!(first.await.unwrap().unwrap()["allow"], true);

    let failing = Arc::new(CapturingSink::new(true));
    fixture
        .hub
        .lock()
        .await
        .set_protected_audit(failing.clone());
    let reused = fixture
        .agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            crate::permission_request("reused", "Write", serde_json::json!({})),
        )
        .await
        .unwrap();
    assert_eq!(reused["allow"], false);
    assert!(fixture.hub.lock().await.pending_permissions.is_empty());

    let records = failing.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision.as_deref(), Some("allow"));
    assert_eq!(
        records[0].decision_source.as_deref(),
        Some("remembered_grant")
    );
}
