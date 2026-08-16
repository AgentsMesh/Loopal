use loopal_protocol::PermissionIntentDigest;
use loopal_view_state::ViewStateReducer;

use super::controller_hub_support::HubHarness;

#[tokio::test]
async fn hub_permission_response_forwards_bound_intent_digest() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let digest = PermissionIntentDigest::from_bytes([0x5a; 32]);
    let send = tokio::spawn(async move {
        controller
            .respond_permission("child", "permission-42", Some(digest), true)
            .await;
    });

    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/permission_response");
    assert_eq!(request["params"]["agent_name"], "child");
    assert_eq!(request["params"]["tool_call_id"], "permission-42");
    assert_eq!(
        request["params"]["permission_intent_digest"],
        digest.to_string()
    );
    assert_eq!(request["params"]["allow"], true);
    hub.respond_ok(&request, serde_json::json!({})).await;
    send.await.unwrap();
}

#[tokio::test]
async fn hub_permission_response_supports_legacy_absent_digest() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let send = tokio::spawn(async move {
        controller
            .respond_permission("main", "permission-legacy", None, false)
            .await;
    });

    let request = hub.read_request().await;
    assert!(request["params"]["permission_intent_digest"].is_null());
    assert_eq!(request["params"]["allow"], false);
    hub.respond_ok(&request, serde_json::json!({})).await;
    send.await.unwrap();
}

#[tokio::test]
async fn hub_question_answer_and_cancel_use_interaction_id() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let answer = tokio::spawn(async move {
        controller
            .respond_question("child", "question-1", vec!["yes".into()])
            .await;
    });
    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/question_response");
    assert_eq!(request["params"]["agent_name"], "child");
    assert_eq!(request["params"]["question_id"], "question-1");
    assert_eq!(request["params"]["response"]["kind"], "answered");
    assert_eq!(request["params"]["response"]["answers"][0], "yes");
    hub.respond_ok(&request, serde_json::json!({})).await;
    answer.await.unwrap();

    let controller = hub.controller.clone();
    let cancel = tokio::spawn(async move {
        controller.cancel_question("child", "question-2").await;
    });
    let request = hub.read_request().await;
    assert_eq!(request["params"]["question_id"], "question-2");
    assert_eq!(request["params"]["response"]["kind"], "cancelled");
    hub.respond_ok(&request, serde_json::json!({})).await;
    cancel.await.unwrap();
}

#[tokio::test]
async fn hub_agent_names_cover_success_malformed_and_rpc_error() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let fetch = tokio::spawn(async move { controller.fetch_agent_names().await });
    let request = hub.read_request().await;
    assert_eq!(request["method"], "hub/list_agents");
    hub.respond_ok(
        &request,
        serde_json::json!({"agents": [
            {"name": "main"}, {"state": "connected"}, {"name": "child"}
        ]}),
    )
    .await;
    assert_eq!(fetch.await.unwrap(), vec!["main", "child"]);

    let controller = hub.controller.clone();
    let malformed = tokio::spawn(async move { controller.fetch_agent_names().await });
    let request = hub.read_request().await;
    hub.respond_ok(&request, serde_json::json!({"agents": "invalid"}))
        .await;
    assert!(malformed.await.unwrap().is_empty());

    let controller = hub.controller.clone();
    let failed = tokio::spawn(async move { controller.fetch_agent_names().await });
    let request = hub.read_request().await;
    hub.respond_error(&request, "list failed").await;
    assert!(failed.await.unwrap().is_empty());
}

#[tokio::test]
async fn hub_view_snapshot_covers_success_malformed_and_rpc_error() {
    let mut hub = HubHarness::new();
    let controller = hub.controller.clone();
    let fetch = tokio::spawn(async move { controller.fetch_view_snapshot("child").await });
    let request = hub.read_request().await;
    assert_eq!(request["method"], "view/snapshot");
    assert_eq!(request["params"]["agent"], "child");
    let snapshot = ViewStateReducer::new("child").snapshot();
    hub.respond_ok(&request, serde_json::to_value(snapshot).unwrap())
        .await;
    let snapshot = fetch.await.unwrap().unwrap();
    assert_eq!(snapshot.state.agent.name, "child");

    let controller = hub.controller.clone();
    let malformed = tokio::spawn(async move { controller.fetch_view_snapshot("child").await });
    let request = hub.read_request().await;
    hub.respond_ok(&request, serde_json::json!({})).await;
    assert!(
        malformed
            .await
            .unwrap()
            .unwrap_err()
            .contains("malformed snapshot")
    );

    let controller = hub.controller.clone();
    let failed = tokio::spawn(async move { controller.fetch_view_snapshot("child").await });
    let request = hub.read_request().await;
    hub.respond_error(&request, "snapshot failed").await;
    assert!(failed.await.unwrap().unwrap_err().contains("view/snapshot"));
}
