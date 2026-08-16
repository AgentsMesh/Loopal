use loopal_protocol::{ControlCommand, UserContent};

use super::controller_hub_support::{HubHarness, disconnected_controller};

#[tokio::test]
async fn hub_control_targets_active_agent() {
    let mut hub = HubHarness::new();
    hub.controller.enter_agent_view("child");
    let controller = hub.controller.clone();
    let send = tokio::spawn(async move {
        controller.switch_model("model-2".into()).await;
    });

    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/control");
    assert_eq!(request["params"]["target"], "child");
    assert_eq!(request["params"]["command"]["ModelSwitch"], "model-2");
    hub.respond_ok(&request, serde_json::json!({})).await;
    send.await.unwrap();
}

#[tokio::test]
async fn hub_route_targets_active_agent() {
    let mut hub = HubHarness::new();
    hub.controller.enter_agent_view("child");
    let controller = hub.controller.clone();
    let send = tokio::spawn(async move {
        controller
            .route_message(UserContent::text_only("hello child"))
            .await;
    });

    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/route");
    assert_eq!(request["params"]["target"]["agent"], "child");
    assert_eq!(request["params"]["content"]["text"], "hello child");
    hub.respond_ok(&request, serde_json::json!({})).await;
    send.await.unwrap();
}

#[tokio::test]
async fn hub_interrupts_cover_active_and_explicit_targets() {
    let mut hub = HubHarness::new();
    hub.controller.enter_agent_view("child");
    hub.controller.interrupt();
    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/interrupt");
    assert_eq!(request["params"]["target"], "child");
    hub.respond_ok(&request, serde_json::json!({})).await;

    hub.controller.interrupt_agent("other");
    let request = hub.read_request().await;
    assert_eq!(request["params"]["target"], "other");
    hub.respond_ok(&request, serde_json::json!({})).await;
}

#[tokio::test]
async fn hub_shutdown_sends_exact_request() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let shutdown = tokio::spawn(async move { controller.shutdown_hub().await });
    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/shutdown");
    assert_eq!(request["params"], serde_json::json!({}));
    hub.respond_ok(&request, serde_json::json!({})).await;
    shutdown.await.unwrap();
}

#[tokio::test]
async fn disconnected_hub_operations_return_without_panicking() {
    let controller = disconnected_controller();
    controller
        .send_control("main".into(), ControlCommand::Clear)
        .await;
    controller
        .route_message(UserContent::text_only("dropped"))
        .await;
    controller
        .respond_permission("main", "permission", None, true)
        .await;
    controller
        .respond_question("main", "question", vec!["answer".into()])
        .await;
    controller.cancel_question("main", "question").await;
    assert!(controller.fetch_agent_names().await.is_empty());
    assert!(controller.fetch_view_snapshot("main").await.is_err());
    controller.interrupt_agent("main");
    tokio::task::yield_now().await;
}
