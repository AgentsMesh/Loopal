use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};
use loopal_session::{ROOT_AGENT, SessionController};
use tokio::sync::mpsc;

use super::controller_hub_support::HubHarness;

fn local_controller() -> SessionController {
    let (control_tx, _) = mpsc::channel(1);
    let (permission_tx, _) = mpsc::channel(1);
    let (question_tx, _) = mpsc::channel(1);
    SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0).0),
    )
}

#[tokio::test]
async fn hub_list_agents_maps_states_and_handles_rpc_failure() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let list = tokio::spawn(async move { controller.list_agents().await });
    let request = hub.read_request().await;
    hub.respond_ok(
        &request,
        serde_json::json!({"agents": [
            {"name": "a", "state": "local"},
            {"name": "b", "state": "connected"},
            {"name": "c", "state": "shadow"},
            {"name": "d", "state": "unexpected"},
            {"name": "missing-state"},
            {"state": "local"}
        ]}),
    )
    .await;
    assert_eq!(
        list.await.unwrap(),
        vec![
            ("a".into(), "local"),
            ("b".into(), "connected"),
            ("c".into(), "shadow"),
            ("d".into(), "unknown"),
        ]
    );

    let controller = hub.controller.clone();
    let list = tokio::spawn(async move { controller.list_agents().await });
    let request = hub.read_request().await;
    hub.respond_error(&request, "list failed").await;
    assert!(list.await.unwrap().is_empty());
}

#[test]
fn repeated_entry_and_root_exit_are_noops() {
    let controller = local_controller();
    assert!(!controller.enter_agent_view(ROOT_AGENT));
    controller.exit_agent_view();
    assert_eq!(controller.lock().active_view, ROOT_AGENT);

    assert!(controller.enter_agent_view("child"));
    controller.exit_agent_view();
    assert_eq!(controller.lock().active_view, ROOT_AGENT);
}

#[test]
fn remote_root_resume_does_not_rebind_local_root_session() {
    let controller = local_controller();
    controller.set_root_session_id("local-session");
    controller.handle_event(AgentEvent::named(
        QualifiedAddress::remote(["remote-hub"], ROOT_AGENT),
        AgentEventPayload::SessionResumed {
            session_id: "remote-session".into(),
            message_count: 1,
        },
    ));

    assert_eq!(
        controller.root_session_id().as_deref(),
        Some("local-session")
    );
}
