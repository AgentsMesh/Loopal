//! ACP control method integration tests: set_mode, set_config_option.

use serde_json::json;

use loopal_test_support::assertions;

use super::e2e_harness::build_acp_harness;

/// Helper: initialize + create session, return session ID.
async fn setup_session(harness: &mut super::e2e_harness::AcpTestHarness) -> String {
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    let resp = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    resp["result"]["sessionId"]
        .as_str()
        .expect("sessionId")
        .to_string()
}

#[tokio::test]
async fn test_set_mode_plan() {
    let mut harness = build_acp_harness(vec![]).await;
    let sid = setup_session(&mut harness).await;

    let resp = harness
        .request(
            "session/set_mode",
            json!({"sessionId": sid, "modeId": "plan"}),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_set_mode_act() {
    let mut harness = build_acp_harness(vec![]).await;
    let sid = setup_session(&mut harness).await;

    let resp = harness
        .request(
            "session/set_mode",
            json!({"sessionId": sid, "modeId": "act"}),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_set_mode_unknown_returns_error() {
    let mut harness = build_acp_harness(vec![]).await;
    let _sid = setup_session(&mut harness).await;

    let resp = harness
        .request(
            "session/set_mode",
            json!({"modeId": "Plan"}), // uppercase → invalid
        )
        .await;
    assert!(
        resp.get("error").is_some(),
        "expected error for non-lowercase modeId: {resp}"
    );
}

#[tokio::test]
async fn test_set_config_option_unknown() {
    let mut harness = build_acp_harness(vec![]).await;
    let _sid = setup_session(&mut harness).await;

    let resp = harness
        .request(
            "session/set_config_option",
            json!({"configId": "unknown_option", "value": "x"}),
        )
        .await;
    assert!(
        resp.get("error").is_some(),
        "expected error for unknown configId: {resp}"
    );
}
