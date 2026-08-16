use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use loopal_mock_llm_lib::{Scenario, serve};
use serde_json::Value;
use tokio::net::TcpListener;

pub struct MockLlmServer {
    base_url: String,
    task: tokio::task::AbortHandle,
    http: reqwest::Client,
}

impl MockLlmServer {
    pub async fn start(scenario: Value, api_key: impl Into<String>) -> Self {
        let scenario = Scenario::from_slice(
            &serde_json::to_vec(&scenario).expect("serialize mock LLM scenario"),
        )
        .expect("valid mock LLM scenario");
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .expect("bind mock LLM server");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("mock LLM address")
        );
        let task = tokio::spawn(serve(listener, scenario, api_key.into())).abort_handle();
        Self {
            base_url,
            task,
            http: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn requests(&self) -> Value {
        self.get("/__mock/requests").await
    }

    pub async fn verify(&self) -> Value {
        self.get("/__mock/verify").await
    }

    pub async fn append_calls(&self, calls: Vec<Value>) -> Value {
        append_calls(&self.http, &self.base_url, calls).await
    }

    async fn get(&self, path: &str) -> Value {
        self.http
            .get(format!("{}{path}", self.base_url))
            .send()
            .await
            .expect("query mock LLM")
            .json()
            .await
            .expect("decode mock LLM response")
    }
}

impl Drop for MockLlmServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn append_mock_calls(base_url: &str, calls: Vec<Value>) -> Value {
    append_calls(&reqwest::Client::new(), base_url, calls).await
}

async fn append_calls(client: &reqwest::Client, base_url: &str, calls: Vec<Value>) -> Value {
    let response = client
        .post(format!("{base_url}/__mock/calls"))
        .json(&serde_json::json!({"calls": calls}))
        .send()
        .await
        .expect("append mock LLM calls");
    let status = response.status();
    let body = response.text().await.expect("read append response");
    assert!(
        status.is_success(),
        "append mock LLM calls failed ({status}): {body}"
    );
    serde_json::from_str(&body).expect("decode append response")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::MockLlmServer;

    #[tokio::test]
    async fn exposes_control_endpoints_and_stops_on_drop() {
        let server = MockLlmServer::start(
            json!({"version": 3, "name": "empty", "calls": []}),
            "test-key",
        )
        .await;

        assert_eq!(server.requests().await, json!([]));
        let state = server.verify().await;
        assert_eq!(state["name"], "empty");
        assert_eq!(state["verified"], true);
        assert!(server.base_url().starts_with("http://127.0.0.1:"));
        assert_eq!(
            server
                .append_calls(vec![json!({
                    "label": "later",
                    "chunks": [{"type": "done"}]
                })])
                .await["remaining"],
            1
        );
    }
}
