//! Integration tests for the AgentsMesh `_loopal/*` ACP control extension:
//! `controlRequest` capability advertisement + `session/control_request`
//! round-trip (parse_loopal_control → send_control → mock agent). Exercises the
//! same ACP → UiSession → Hub → Agent path as production.

use serde_json::json;

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::{AcpTestHarness, build_acp_harness};

async fn setup_session(harness: &mut AcpTestHarness) -> String {
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
async fn test_initialize_advertises_control_request() {
    let mut harness = build_acp_harness(vec![]).await;
    let resp = harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    assert_eq!(
        resp["result"]["agentsmeshExtensions"]["controlRequest"], true,
        "initialize must advertise controlRequest so the runner routes Loopal control: {resp}"
    );
}

#[tokio::test]
async fn test_control_request_compact_ok() {
    let mut harness = build_acp_harness(vec![]).await;
    let sid = setup_session(&mut harness).await;
    let resp = harness
        .request(
            "session/control_request",
            json!({"sessionId": sid, "subtype": "loopal.compact", "params": {}}),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_control_request_bgtask_kill_running_process() {
    let calls = vec![
        chunks::tool_turn(
            "tc-bg",
            "Bash",
            json!({
                "command": "sleep 60",
                "run_in_background": true,
                "description": "acp background fixture"
            }),
        ),
        chunks::text_turn("Background process started."),
    ];
    let mut harness = build_acp_harness(calls).await;
    let sid = setup_session(&mut harness).await;
    let (prompt, notifications) = harness
        .request_with_notifications(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "start it"}]}),
        )
        .await;
    assertions::assert_json_rpc_ok(&prompt);
    let process_id = notifications
        .iter()
        .find(|notification| notification["method"] == "_loopal/bgTask.spawned")
        .and_then(|notification| notification["params"]["data"]["id"].as_str())
        .expect("background spawn notification");

    let resp = harness
        .request(
            "session/control_request",
            json!({
                "sessionId": sid,
                "subtype": "loopal.bgTaskKill",
                "params": {"id": process_id}
            }),
        )
        .await;
    assertions::assert_json_rpc_ok(&resp);
}

#[tokio::test]
async fn test_control_request_bgtask_kill_rejects_missing_process() {
    let mut harness = build_acp_harness(vec![]).await;
    let sid = setup_session(&mut harness).await;
    let resp = harness
        .request(
            "session/control_request",
            json!({"sessionId": sid, "subtype": "loopal.bgTaskKill", "params": {"id": "bg1"}}),
        )
        .await;
    assertions::assert_json_rpc_error(&resp, -32603);
    assert!(
        resp["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Process not found: bg1")),
        "missing process rejection should preserve the runtime reason: {resp}"
    );
}

#[tokio::test]
async fn test_control_request_unknown_subtype_rejected() {
    let mut harness = build_acp_harness(vec![]).await;
    let sid = setup_session(&mut harness).await;
    let resp = harness
        .request(
            "session/control_request",
            json!({"sessionId": sid, "subtype": "loopal.bogus", "params": {}}),
        )
        .await;
    assertions::assert_json_rpc_error(&resp, -32601);
}

#[tokio::test]
async fn test_session_new_replays_loopal_snapshot() {
    let mut harness = build_acp_harness(vec![]).await;
    harness
        .request("initialize", json!({"protocolVersion": 1}))
        .await;
    // session/new fires replay_loopal_snapshot AFTER responding (post bootstrap
    // drain). The replay always emits _loopal/crons (even empty), so observing it
    // on the stream proves the cold-start path runs end-to-end: drain returns on
    // the agent's AwaitingInput → fetch view/snapshot → notify _loopal/*.
    let _ = harness.request("session/new", json!({"cwd": "/tmp"})).await;
    let crons = harness.read_until_method("_loopal/crons").await;
    assert!(
        crons.is_some(),
        "expected _loopal/crons notification from snapshot replay"
    );
    assert!(crons.unwrap()["params"]["data"]["crons"].is_array());
}
