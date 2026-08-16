use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_mcp::HubMcpClient;
use serde_json::Value;

pub(crate) struct MockHubClient {
    responses: Mutex<Vec<(String, Value)>>,
    requests: Mutex<Vec<(String, Value)>>,
}

impl MockHubClient {
    pub(crate) fn new(responses: Vec<(&str, Value)>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(method, value)| (method.to_string(), value))
                    .collect(),
            ),
            requests: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn requests(&self) -> Vec<(String, Value)> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl HubMcpClient for MockHubClient {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.requests
            .lock()
            .unwrap()
            .push((method.to_string(), params));
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(format!("no mock response for {method}"));
        }
        let (expected, response) = responses.remove(0);
        assert_eq!(expected, method);
        Ok(response)
    }
}
