use std::collections::VecDeque;

use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, Provider};
use serde_json::json;
use tracing::Instrument;

use super::AnthropicProvider;
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
            model = %params.model, url = %format!("{}/v1/messages", self.base_url),
            messages = params.messages.len(), tools = params.tools.len(),
            max_tokens = params.max_tokens,
            body_bytes = body.to_string().len(),
            "API request"
        );

        let http_span = tracing::info_span!("http_request", gen_ai.system = "anthropic");
        let (client, client_gen) = self.client.get();
        let response = client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .instrument(http_span)
            .await
            .map_err(|e| {
                self.client.report_network_error(client_gen);
                ProviderError::Http(format!("{e:#}"))
            })?;
        self.client.report_success(client_gen);

        let status = response.status();
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
        let normalized = loopal_message::normalize_messages(&params.messages);
        let normalized_params = ChatParams {
            messages: normalized,
            ..params.clone()
        };
        let finalized = self.finalize_messages(&normalized_params).into_owned();
        let final_params = ChatParams {
            messages: finalized,
            ..normalized_params
        };
        let messages = self.build_messages(&final_params);
        let tools = self.build_tools(&final_params);

        let mut body = json!({
            "model": final_params.model,
            "max_tokens": final_params.max_tokens,
            "stream": true,
            "messages": messages,
        });
        if !final_params.system_prompt.is_empty() {
            body["system"] = json!([{
                "type": "text",
                "text": final_params.system_prompt,
                "cache_control": {"type": "ephemeral"}
            }]);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(temp) = final_params.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(ref thinking_config) = final_params.thinking {
            body["thinking"] =
                thinking::to_anthropic_thinking(thinking_config, final_params.max_tokens);
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
        if status.as_u16() == 429 {
            let retry_after_ms = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<f64>().ok())
                .map(|secs| (secs * 1000.0) as u64)
                .unwrap_or(30_000);
            tracing::warn!(retry_after_ms, "rate limited by API");
            return ProviderError::RateLimited { retry_after_ms }.into();
        }
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read body".into());
        tracing::error!(status = status.as_u16(), body = %text, "API error");
        ProviderError::Api {
            status: status.as_u16(),
            message: text,
        }
        .into()
    }
}
