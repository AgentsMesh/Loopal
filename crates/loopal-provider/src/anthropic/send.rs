use std::collections::VecDeque;

use loopal_error::LoopalError;
use loopal_provider_api::{ChatParams, ChatStream};
use serde_json::json;
use tracing::Instrument;

use super::AnthropicProvider;
use super::capability;
use super::stream::{
    AnthropicStream, ServerToolAccumulator, ThinkingAccumulator, ToolUseAccumulator,
};
use super::thinking;
use crate::sse::SseStream;

impl AnthropicProvider {
    pub(super) async fn do_stream_chat(
        &self,
        params: &ChatParams,
    ) -> Result<ChatStream, LoopalError> {
        let body = self.build_request_body(params);
        tracing::info!(
            model = %params.model, provider = "anthropic",
            messages = params.turns.len(), tools = params.tools.len(),
            max_tokens = params.max_tokens,
            body_bytes = body.to_string().len(),
            "API request"
        );

        let endpoint = format!("{}/v1/messages", self.base_url);
        let http_span = crate::http_telemetry::request_span("anthropic", &endpoint);
        let (client, client_gen) = self.client.get();
        let response = client
            .post(endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .instrument(http_span.clone())
            .await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                crate::http_telemetry::record_transport_error(&http_span);
                self.client.report_network_error(client_gen);
                return Err(crate::safe_diagnostics::network_error("anthropic", &e).into());
            }
        };
        self.client.report_success(client_gen);

        let status = response.status();
        crate::http_telemetry::record_response(&http_span, status);
        tracing::info!(status = status.as_u16(), "API response");
        if !status.is_success() {
            self.dump_failed_request(&body, params, status);
            return Err(self.handle_error_response(response, status).await);
        }

        let sse = SseStream::from_response(response);
        Ok(Box::pin(AnthropicStream {
            inner: Box::pin(sse),
            tool_state: ToolUseAccumulator::default(),
            thinking_state: ThinkingAccumulator::default(),
            server_tool_state: ServerToolAccumulator::default(),
            buffer: VecDeque::new(),
        }))
    }

    fn build_request_body(&self, params: &ChatParams) -> serde_json::Value {
        // reason: domain SSOT — fold Turns directly into wire JSON.
        // 5 invariants (alternation / id pairing / tool_result-before-text /
        // parallel ordering / server pairing) are statically encoded in the
        // Turn shape, so no normalize / sanitize pass is needed.
        let messages = self.build_messages_json_from_turns(params);
        let tools = self.build_tools(params);

        let mut body = json!({
            "model": params.model,
            "max_tokens": params.max_tokens,
            "stream": true,
            "messages": messages,
        });
        if !params.system_prompt.is_empty() {
            body["system"] = json!([{
                "type": "text",
                "text": params.system_prompt,
                "cache_control": {"type": "ephemeral"}
            }]);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(temp) = params.temperature
            && capability::supports_temperature(&params.model)
        {
            body["temperature"] = json!(temp);
        } else if params.temperature.is_some() {
            tracing::debug!(
                model = %params.model,
                "dropping temperature for model not on Anthropic temperature allowlist"
            );
        }
        if let Some(ref thinking_config) = params.thinking {
            body["thinking"] = thinking::to_anthropic_thinking(thinking_config, params.max_tokens);
            if let Some(output_config) = thinking::to_anthropic_output_config(thinking_config) {
                body["output_config"] = output_config;
            }
        }
        body
    }

    fn dump_failed_request(
        &self,
        body: &serde_json::Value,
        params: &ChatParams,
        status: reqwest::StatusCode,
    ) {
        let Some(ref dump_dir) = params.debug_dump_dir else {
            return;
        };
        let _ = std::fs::create_dir_all(dump_dir);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = dump_dir.join(format!("api_error_{status}_{ts}.json"));
        let _ = std::fs::write(&path, body.to_string());
        tracing::warn!(path = %path.display(), "dumped failed request body");
    }

    pub(super) async fn handle_error_response(
        &self,
        response: reqwest::Response,
        status: reqwest::StatusCode,
    ) -> LoopalError {
        let retry_after_ms = crate::retry_after::from_headers(response.headers());
        if status.as_u16() == 429 {
            let error = crate::retry_after::provider_error(
                status.as_u16(),
                "rate limited by API".into(),
                retry_after_ms,
            );
            let retry_after_ms = error.retry_after_ms().unwrap_or_default();
            tracing::warn!(retry_after_ms, "rate limited by API");
            return error.into();
        }
        let text = crate::safe_diagnostics::response_error_message(
            "anthropic",
            response,
            &[&self.api_key, &self.base_url],
        )
        .await;
        tracing::error!(status = status.as_u16(), "API error");
        crate::retry_after::provider_error(status.as_u16(), text, retry_after_ms).into()
    }
}
