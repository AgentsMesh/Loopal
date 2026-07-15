use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use futures::StreamExt;
use loopal_mock_llm_lib::{Scenario, serve};
use loopal_provider_api::{ChatParams, Provider, StreamChunk, ThinkingConfig};
use loopal_tool_api::ToolDefinition;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub const API_KEY: &str = "contract-secret-key";

pub async fn start(scenario: Value) -> (String, JoinHandle<anyhow::Result<()>>) {
    let scenario = Scenario::from_slice(&serde_json::to_vec(&scenario).unwrap()).unwrap();
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(serve(listener, scenario, API_KEY.into()));
    (format!("http://{address}"), task)
}

pub fn params() -> ChatParams {
    ChatParams {
        model: "deepseek-reasoner".into(),
        turns: vec![loopal_turn::Turn::single_user_prompt(
            "wire contract marker",
        )],
        system_prompt: "System contract marker".into(),
        tools: vec![ToolDefinition {
            name: "Read".into(),
            description: "Read a fixture".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"file_path": {"type": "string"}}
            }),
        }],
        max_tokens: 256,
        temperature: None,
        thinking: Some(ThinkingConfig::Budget { tokens: 64 }),
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

pub async fn collect(provider: &dyn Provider) -> Vec<StreamChunk> {
    provider
        .stream_chat(&params())
        .await
        .unwrap()
        .map(|item| item.unwrap())
        .collect()
        .await
}

pub fn semantic_call(protocol: &str) -> Value {
    json!({
        "expect": {
            "protocol": protocol, "model": "deepseek-reasoner",
            "userContains": "wire contract marker", "minTools": 1
        },
        "chunks": [
            {"type": "thinking", "text": "reasoning wire"},
            {"type": "thinking_signature", "signature": "reasoning-signature"},
            {"type": "text", "text": "hello wire"},
            {"type": "tool_use", "id": "read-1", "name": "Read",
             "input": {"file_path": "README.md"}},
            {"type": "usage", "input": 12, "output": 7, "thinking": 3,
             "cache_read": 2},
            {"type": "done", "reason": "end_turn"}
        ]
    })
}
