use serde_json::json;

use super::helpers::{API_KEY, start};

#[tokio::test]
async fn every_protocol_rejects_missing_or_incomplete_auth() {
    let (base, task) = start(json!({"version": 2, "name": "auth", "calls": []})).await;
    let client = reqwest::Client::new();
    let cases = [
        client.post(format!("{base}/v1/messages")).json(&json!({})),
        client
            .post(format!("{base}/v1/messages"))
            .header("x-api-key", API_KEY)
            .json(&json!({})),
        client
            .post(format!("{base}/v1/responses"))
            .header("authorization", "Basic invalid")
            .json(&json!({})),
        client
            .post(format!("{base}/v1/chat/completions"))
            .header("authorization", "Bearer wrong")
            .json(&json!({})),
        client
            .post(format!("{base}/models/test:streamGenerateContent?alt=sse"))
            .json(&json!({})),
        client
            .post(format!(
                "{base}/models/test:streamGenerateContent?key={API_KEY}&alt=json"
            ))
            .json(&json!({})),
    ];
    let expected = [401, 400, 401, 401, 401, 400];
    for (request, status) in cases.into_iter().zip(expected) {
        assert_eq!(request.send().await.unwrap().status().as_u16(), status);
    }
    let requests: serde_json::Value = reqwest::get(format!("{base}/__mock/requests"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(requests, json!([]));
    assert!(!requests.to_string().contains(API_KEY));
    task.abort();
}

#[tokio::test]
async fn close_before_headers_is_a_counted_transport_fault() {
    let (base, task) = start(json!({
        "version": 2,
        "name": "transport-fault",
        "calls": [{
            "expect": {"protocol": "anthropic"},
            "closeBeforeHeaders": true
        }]
    }))
    .await;
    let response = reqwest::Client::new()
        .post(format!("{base}/v1/messages"))
        .header("x-api-key", API_KEY)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({"model": "test", "messages": []}))
        .send()
        .await;
    assert!(response.is_err());
    let state: serde_json::Value = reqwest::get(format!("{base}/__mock/state"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state["scriptedDisconnects"], 1);
    assert_eq!(state["clientDisconnects"], 0);
    assert_eq!(state["verified"], true);
    task.abort();
}

#[tokio::test]
async fn journal_marks_anthropic_error_tool_results_without_content_leakage() {
    let (base, task) = start(json!({
        "version": 2,
        "calls": [{
            "expect": {"protocol": "anthropic", "toolResultId": "failed-1"},
            "chunks": [{"type": "done"}]
        }]
    }))
    .await;
    let status = reqwest::Client::new()
        .post(format!("{base}/v1/messages"))
        .header("x-api-key", API_KEY)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "test", "stream": true,
            "messages": [{"role": "user", "content": [{
                "type": "tool_result", "tool_use_id": "failed-1",
                "content": "private failure detail", "is_error": true
            }]}]
        }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 200);
    let requests: serde_json::Value = reqwest::get(format!("{base}/__mock/requests"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(requests[0]["toolResultIds"], json!(["failed-1"]));
    assert_eq!(requests[0]["toolResultErrorIds"], json!(["failed-1"]));
    assert!(!requests.to_string().contains("private failure detail"));
    task.abort();
}
