//! Load mock LLM responses from a JSON file for system integration tests.
//!
//! File format: array of calls, each call is array of chunks.
//! Chunk types: `{"type":"text","text":"..."}`, `{"type":"done"}`,
//! `{"type":"usage"}`, `{"type":"tool_use","id":"...","name":"...","input":{}}`.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};

/// Load a mock provider from a JSON fixture file.
pub fn load_mock_provider(path: &str) -> anyhow::Result<Arc<dyn Provider>> {
    let content = std::fs::read_to_string(path)?;
    let raw: Vec<Vec<serde_json::Value>> = serde_json::from_str(&content)?;
    let calls: Vec<Vec<Result<StreamChunk, LoopalError>>> = raw
        .into_iter()
        .map(|call| call.into_iter().map(parse_chunk).collect())
        .collect();
    Ok(Arc::new(JsonMockProvider {
        calls: std::sync::Mutex::new(VecDeque::from(calls)),
    }))
}

fn parse_chunk(v: serde_json::Value) -> Result<StreamChunk, LoopalError> {
    match v["type"].as_str().unwrap_or("") {
        "text" => Ok(StreamChunk::Text {
            text: v["text"].as_str().unwrap_or("").into(),
        }),
        "tool_use" => Ok(StreamChunk::ToolUse {
            id: v["id"].as_str().unwrap_or("tc-1").into(),
            name: v["name"].as_str().unwrap_or("").into(),
            input: v.get("input").cloned().unwrap_or_default(),
        }),
        "usage" => Ok(StreamChunk::Usage {
            input_tokens: v["input"].as_u64().unwrap_or(10) as u32,
            output_tokens: v["output"].as_u64().unwrap_or(5) as u32,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        }),
        "done" => Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
        _ => Ok(StreamChunk::Text {
            text: String::new(),
        }),
    }
}

struct JsonMockProvider {
    calls: std::sync::Mutex<VecDeque<Vec<Result<StreamChunk, LoopalError>>>>,
}

#[async_trait]
impl Provider for JsonMockProvider {
    fn name(&self) -> &str {
        "anthropic" // Match default model prefix for resolve_provider
    }

    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let chunks = self
            .calls
            .lock()
            .expect("mock provider mutex poisoned")
            .pop_front()
            .unwrap_or_default();
        let stream = futures::stream::iter(chunks);
        Ok(Box::pin(stream))
    }
}
