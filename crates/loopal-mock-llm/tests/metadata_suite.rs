use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use loopal_mock_llm_lib::{Scenario, serve};
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[test]
fn validates_v3_label_and_metadata_schema() {
    let invalid = [
        json!({"version": 2, "calls": [{"label": "old", "chunks": []}]}),
        json!({"version": 2, "calls": [{"expect": {
            "requestMetadata": [{"path": "/attempt", "exists": true}]
        }, "chunks": []}]}),
        json!({"version": 3, "calls": [{"label": " ", "chunks": []}]}),
        json!({"version": 3, "calls": [{"label": "line\nbreak", "chunks": []}]}),
        json!({"version": 3, "calls": [{"expect": {"requestMetadata": []}, "chunks": []}]}),
        json!({"version": 3, "calls": [{"expect": {"requestMetadata": [
            {"path": "/attempt"}
        ]}, "chunks": []}]}),
        json!({"version": 3, "calls": [{"expect": {"requestMetadata": [
            {"path": "attempt", "exists": true}
        ]}, "chunks": []}]}),
        json!({"version": 3, "calls": [{"expect": {"requestMetadata": [
            {"path": "/attempt", "exists": false, "equals": 1}
        ]}, "chunks": []}]}),
        json!({"version": 3, "calls": [{"expect": {"requestMetadata": [
            {"path": "/attempt", "unknown": true}
        ]}, "chunks": []}]}),
    ];
    for value in invalid {
        let error = Scenario::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert!(!error.to_string().is_empty(), "accepted {value}");
    }
}

#[tokio::test]
async fn metadata_predicates_select_calls_and_journal_only_labels() {
    let (base, task) = start(json!({
        "version": 3,
        "calls": [
            {"label": "attempt one", "expect": {"requestMetadata": [
                {"path": "", "exists": true},
                {"path": "/workflow/run", "equals": "run-secret"},
                {"path": "/workflow/attempt", "equals": 1},
                {"path": "/workflow/node", "contains": "build"},
                {"path": "/workflow/node", "excludes": "deploy"},
                {"path": "/workflow/missing", "exists": false}
            ]}, "chunks": [{"type": "text", "text": "one"}, {"type": "done"}]},
            {"label": "attempt two", "expect": {"requestMetadata": [
                {"path": "/workflow/attempt", "equals": 2}
            ]}, "chunks": [{"type": "text", "text": "two"}, {"type": "done"}]}
        ]
    }))
    .await;
    let body = post(
        &base,
        json!({"metadata": {"workflow": {
            "run": "run-secret", "attempt": 1, "node": "build-node"
        }}}),
    )
    .await;
    assert!(body.contains("one"));
    let journal: Value = reqwest::get(format!("{base}/__mock/requests"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(journal[0]["callLabel"], "attempt one");
    let encoded = journal.to_string();
    assert!(!encoded.contains("run-secret"));
    assert!(!encoded.contains("build-node"));
    task.abort();
}

#[tokio::test]
async fn metadata_only_expectation_is_not_a_wildcard_and_errors_are_redacted() {
    let secret = "raw-metadata-secret";
    let (base, task) = start(json!({
        "version": 3,
        "calls": [{
            "label": "workflow attempt two",
            "expect": {"requestMetadata": [{"path": "/attempt", "equals": 2}]},
            "chunks": [{"type": "done"}]
        }]
    }))
    .await;
    let response = post_response(
        &base,
        json!({"metadata": {
            "attempt": 1, "token": secret
        }}),
    )
    .await;
    assert_eq!(response.status(), 409);
    let error = response.text().await.unwrap();
    assert!(error.contains("workflow attempt two"));
    assert!(error.contains("/attempt"));
    assert!(!error.contains(secret));
    let journal: Value = reqwest::get(format!("{base}/__mock/requests"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(journal[0]["matched"], false);
    assert!(journal[0]["callLabel"].is_null());
    assert!(
        journal[0]["matchErrors"][0]
            .as_str()
            .unwrap()
            .contains("workflow attempt two")
    );
    assert!(!journal.to_string().contains(secret));
    let state: Value = reqwest::get(format!("{base}/__mock/state"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state["remaining"], 1);
    task.abort();
}

async fn start(scenario: Value) -> (String, tokio::task::JoinHandle<()>) {
    let scenario = Scenario::from_slice(&serde_json::to_vec(&scenario).unwrap()).unwrap();
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let _ = serve(listener, scenario, "test-key".into()).await;
    });
    (format!("http://{address}"), task)
}

async fn post(base: &str, body: Value) -> String {
    post_response(base, body).await.text().await.unwrap()
}

async fn post_response(base: &str, body: Value) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("{base}/v1/messages"))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .unwrap()
}
