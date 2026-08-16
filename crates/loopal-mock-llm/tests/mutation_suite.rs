use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use loopal_mock_llm_lib::{Scenario, serve};
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[tokio::test]
async fn appends_scripted_calls_atomically_through_control_endpoint() {
    let scenario = Scenario::from_slice(
        &serde_json::to_vec(&json!({"version": 3, "name": "mutation", "calls": []})).unwrap(),
    )
    .unwrap();
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(serve(listener, scenario, "test-key".into()));
    let base = format!("http://{address}");
    let client = reqwest::Client::new();

    let appended = client
        .post(format!("{base}/__mock/calls"))
        .json(&json!({"calls": [{
            "label": "dynamic", "expect": {"userContains": "dynamic marker"},
            "chunks": [{"type": "text", "text": "dynamic result"}, {"type": "done"}]
        }]}))
        .send()
        .await
        .unwrap();
    assert_eq!(appended.status(), 200);
    assert_eq!(appended.json::<Value>().await.unwrap()["remaining"], 1);

    let body = completion(&client, &base, "dynamic marker").await;
    assert_eq!(body.status(), 200);
    assert!(body.text().await.unwrap().contains("dynamic result"));
    let journal: Value = client
        .get(format!("{base}/__mock/requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(journal[0]["callLabel"], "dynamic");

    let rejected = client
        .post(format!("{base}/__mock/calls"))
        .json(&json!({"calls": [
            {"label": "must-not-stick", "chunks": [{"type": "done"}]},
            {"label": "invalid", "chunks": [{"type": "done", "typo": true}]}
        ]}))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 400);
    let state: Value = client
        .get(format!("{base}/__mock/state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        state["remaining"], 0,
        "invalid batch partially mutated state: {state}"
    );
    task.abort();
}

async fn completion(client: &reqwest::Client, base: &str, text: &str) -> reqwest::Response {
    client
        .post(format!("{base}/v1/messages"))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({"messages": [{"role": "user", "content": text}]}))
        .send()
        .await
        .unwrap()
}
