use std::sync::{Arc, Mutex};

use loopal_tool_api::{MemoryChannel, PermissionLevel, Tool, ToolContext};
use serde_json::json;

use loopal_memory::MemoryTool;

/// Mock channel that records observations.
struct RecordingChannel(Mutex<Vec<String>>);

impl RecordingChannel {
    fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(Vec::new())))
    }
    fn observations(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl MemoryChannel for RecordingChannel {
    fn try_send(&self, observation: String) -> Result<(), String> {
        self.0.lock().unwrap().push(observation);
        Ok(())
    }
}

/// Mock channel that always rejects (simulates full channel).
struct FullChannel;

impl MemoryChannel for FullChannel {
    fn try_send(&self, _observation: String) -> Result<(), String> {
        Err("channel full".into())
    }
}

fn make_ctx(channel: Option<Arc<dyn MemoryChannel>>) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        backend,
        session_id: "test".into(),
        shared: None,
        memory_channel: channel,
        output_tail: None,
    }
}

#[test]
fn test_tool_metadata() {
    let tool = MemoryTool;
    assert_eq!(tool.name(), "Memory");
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
    assert!(tool.description().contains("cross-session"));

    let schema = tool.parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "observation"));
}

#[tokio::test]
async fn test_valid_observation_sends_to_channel() {
    let channel = RecordingChannel::new();
    let ctx = make_ctx(Some(channel.clone()));
    let tool = MemoryTool;

    let result = tool
        .execute(json!({"observation": "user prefers bun"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(result.content, "Noted.");
    assert_eq!(channel.observations(), vec!["user prefers bun"]);
}

#[tokio::test]
async fn test_no_channel_returns_error() {
    let ctx = make_ctx(None);
    let tool = MemoryTool;

    let result = tool
        .execute(json!({"observation": "something"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("not enabled"));
}

#[tokio::test]
async fn test_empty_observation_returns_error() {
    let channel = RecordingChannel::new();
    let ctx = make_ctx(Some(channel.clone()));
    let tool = MemoryTool;

    let result = tool
        .execute(json!({"observation": "  "}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("empty"));
    assert!(channel.observations().is_empty());
}

#[tokio::test]
async fn test_missing_observation_returns_error() {
    let channel = RecordingChannel::new();
    let ctx = make_ctx(Some(channel.clone()));
    let tool = MemoryTool;

    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err()); // InvalidInput error
}

#[tokio::test]
async fn test_channel_full_returns_error() {
    let ctx = make_ctx(Some(Arc::new(FullChannel)));
    let tool = MemoryTool;

    let result = tool
        .execute(json!({"observation": "something"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("channel full"));
}
