use loopal_mock_llm_lib::{MockResponse, Scenario, SseAction, plan_sse, serve};
use serde_json::json;
use std::net::Ipv4Addr;
use tokio::net::TcpListener;

mod metadata_suite;
mod mutation_suite;

#[test]
fn parses_legacy_and_request_aware_scenarios() {
    let legacy = Scenario::from_slice(br#"[[{"type":"text","text":"ok"}]]"#).unwrap();
    assert_eq!(legacy.name, "legacy");
    assert_eq!(legacy.remaining(), 1);
    let scenario = Scenario::from_slice(
        serde_json::to_string(&json!({
            "version": 1,
            "name": "contract",
            "calls": [{
                "expect": {
                    "model": "claude-opus-4-8", "userContains": "hello",
                    "thinkingEnabled": false, "imageBlockCount": 0,
                    "assistantBlockTypes": ["text"], "serverBlockCount": 0
                },
                "chunks": [{"type": "done", "reason": "max_tokens"}]
            }]
        }))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(scenario.name, "contract");
    assert_eq!(scenario.remaining(), 1);
}

#[test]
fn rejects_scenario_schema_typos() {
    let invalid = [
        json!({"version": "1", "calls": []}),
        json!({"version": 1, "calls": [], "unexpected": true}),
        json!({"calls": [{
            "expect": {"userContain": "marker"},
            "chunks": [{"type": "done"}]
        }]}),
        json!({"calls": [{
            "chunks": [{"type": "text", "txet": "misspelled"}]
        }]}),
        json!({"version": 2, "calls": [{
            "expect": {"protocol": "openai_chat"}, "chunks": []
        }]}),
    ];
    for value in invalid {
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(Scenario::from_slice(&bytes).is_err(), "accepted {value}");
    }
}

#[test]
fn anthropic_usage_and_stop_share_the_final_delta() {
    let response = MockResponse {
        chunks: vec![
            json!({"type": "usage", "input": 12, "output": 7}),
            json!({"type": "done"}),
        ],
        ..MockResponse::default()
    };
    let deltas: Vec<_> = plan_sse(&response)
        .unwrap()
        .into_iter()
        .filter_map(|action| match action {
            SseAction::Event(data) if data.contains("\"message_delta\"") => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(deltas.len(), 1);
    assert!(deltas[0].contains("\"output_tokens\":7"));
    assert!(deltas[0].contains("\"stop_reason\":\"end_turn\""));
}

#[tokio::test]
async fn serves_real_anthropic_sse_and_redacted_journal() {
    let scenario = Scenario::from_slice(serde_json::to_string(&json!({
        "version": 1,
        "name": "wire",
        "calls": [{
            "expect": {
                "userContains": "contract marker", "minTools": 1,
                "thinkingEnabled": false
            },
            "chunks": [
                {"type": "thinking", "text": "checking"},
                {"type": "thinking_signature", "signature": "sig"},
                {"type": "tool_use", "id": "read-1", "name": "Read", "input": {"file_path": "README.md"}},
                {"type": "usage", "input": 12, "output": 7},
                {"type": "done"}
            ]
        }]
    })).unwrap().as_bytes()).unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(serve(listener, scenario, "test-key".into()));
    let client = reqwest::Client::new();
    let body = client
        .post(format!("http://{address}/v1/messages"))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "claude-opus-4-8", "stream": true,
            "messages": [{"role": "user", "content": "contract marker"}],
            "tools": [{"name": "Read"}]
        }))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("thinking_delta"));
    assert!(body.contains("input_json_delta"));
    assert!(body.contains("message_stop"));
    let requests: serde_json::Value = client
        .get(format!("http://{address}/__mock/requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(requests[0]["apiKeyPresent"], true);
    assert_eq!(requests[0]["lastUserText"], "contract marker");
    assert!(!requests.to_string().contains("test-key"));
    let state: serde_json::Value = client
        .get(format!("http://{address}/__mock/verify"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state["verified"], true);
    task.abort();
}

#[tokio::test]
async fn unmatched_requests_are_recorded_without_consuming_the_call() {
    let scenario = Scenario::from_slice(
        serde_json::to_string(&json!({
            "name": "strict",
            "calls": [{
                "expect": {"userContains": "expected"},
                "chunks": [{"type": "done"}]
            }]
        }))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(serve(listener, scenario, "test-key".into()));
    let client = reqwest::Client::new();
    for (text, status) in [("wrong", 409), ("expected", 200)] {
        let response = client
            .post(format!("http://{address}/v1/messages"))
            .header("x-api-key", "test-key")
            .header("anthropic-version", "2023-06-01")
            .json(&json!({"messages": [{"role": "user", "content": text}]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
    }
    let requests: serde_json::Value = client
        .get(format!("http://{address}/__mock/requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(requests[0]["sequence"], 1);
    assert_eq!(requests[0]["matched"], false);
    assert_eq!(requests[1]["sequence"], 2);
    assert_eq!(requests[1]["matched"], true);
    let state: serde_json::Value = client
        .get(format!("http://{address}/__mock/state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state["remaining"], 0);
    assert_eq!(state["unmatchedRequests"], 1);
    assert_eq!(state["verified"], false);
    task.abort();
}
