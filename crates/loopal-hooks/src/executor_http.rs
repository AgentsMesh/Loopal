//! HTTP hook executor — POSTs JSON to a webhook endpoint.
//!
//! Uses reqwest to send hook input as JSON body. HTTP status maps to exit code:
//! - 2xx → exit 0 (success), response body as stdout
//! - 4xx/5xx → exit 1 (error), response body as stderr

use std::time::Duration;

use loopal_error::HookError;

use crate::executor::{HookExecutor, RawHookOutput};

/// Executes a hook by POSTing JSON to a URL and interpreting the response.
pub struct HttpExecutor {
    pub url: String,
    pub headers: std::collections::HashMap<String, String>,
    pub timeout: Duration,
}

#[async_trait::async_trait]
impl HookExecutor for HttpExecutor {
    async fn execute(&self, input: serde_json::Value) -> Result<RawHookOutput, HookError> {
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

        let mut req = client.post(&self.url).json(&input);
        for (key, value) in &self.headers {
            req = req.header(key.as_str(), value.as_str());
        }

        let response = req
            .send()
            .await
            .map_err(|e| HookError::ExecutionFailed(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| HookError::ExecutionFailed(format!("HTTP body read failed: {e}")))?;

        if status.is_success() {
            Ok(RawHookOutput {
                exit_code: 0,
                stdout: body,
                stderr: String::new(),
            })
        } else {
            Ok(RawHookOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: body,
            })
        }
    }
}
