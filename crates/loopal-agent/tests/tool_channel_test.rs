//! End-to-end integration tests for Channel tool routing through MessageRouter.

use std::sync::Arc;

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_agent::tools::channel::ChannelTool;
use loopal_kernel::Kernel;
use loopal_types::config::Settings;
use loopal_types::event::AgentEvent;
use loopal_types::tool::{Tool, ToolContext};
use tokio::sync::{Mutex, mpsc};

async fn make_ctx(agent_name: &str) -> (Arc<AgentShared>, ToolContext) {
    let (event_tx, _) = mpsc::channel::<AgentEvent>(64);
    let router = Arc::new(MessageRouter::new(event_tx));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let tmp = std::env::temp_dir().join(format!(
        "la_ch_test_{}_{}", agent_name, std::process::id()
    ));
    let shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        registry: Arc::new(Mutex::new(AgentRegistry::new())),
        task_store: Arc::new(TaskStore::new(tmp)),
        router,
        cwd: std::env::temp_dir(),
        depth: 0,
        max_depth: 3,
        agent_name: agent_name.to_string(),
        parent_event_tx: None,
        cancel_token: None,
    });

    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared.clone());
    let ctx = ToolContext {
        cwd: std::env::temp_dir(),
        session_id: "test".to_string(),
        shared: Some(shared_any),
    };
    (shared, ctx)
}

#[tokio::test]
async fn test_channel_subscribe_and_list() {
    let (_shared, ctx) = make_ctx("agent-a").await;
    let tool = ChannelTool;

    let result = tool.execute(serde_json::json!({
        "operation": "subscribe",
        "channel": "updates"
    }), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("#updates"));

    let result = tool.execute(serde_json::json!({
        "operation": "list"
    }), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("#updates"));
}

#[tokio::test]
async fn test_channel_publish_stores_in_history() {
    let (shared, ctx) = make_ctx("publisher").await;
    let tool = ChannelTool;

    // Subscribe "subscriber" to "news" channel (no mailbox needed — pull-only)
    shared.router.subscribe("news", "subscriber").await;

    // Publish from "publisher"
    let result = tool.execute(serde_json::json!({
        "operation": "publish",
        "channel": "news",
        "message": "breaking update"
    }), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("1 subscriber"));
    assert!(result.content.contains("Channel.read"));

    // Verify message is in channel history via read
    let messages = shared.router.read_channel("news", 0).await;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "breaking update");
    assert_eq!(messages[0].from, "publisher");
}

#[tokio::test]
async fn test_channel_read_history() {
    let (_shared, ctx) = make_ctx("reader").await;
    let tool = ChannelTool;

    // Subscribe and publish
    tool.execute(serde_json::json!({
        "operation": "subscribe",
        "channel": "logs"
    }), &ctx).await.unwrap();

    // Publish does not deliver to self, but records history
    tool.execute(serde_json::json!({
        "operation": "publish",
        "channel": "logs",
        "message": "log entry 1"
    }), &ctx).await.unwrap();

    let result = tool.execute(serde_json::json!({
        "operation": "read",
        "channel": "logs",
        "limit": 10
    }), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("log entry 1"));
}

#[tokio::test]
async fn test_channel_unsubscribe() {
    let (_shared, ctx) = make_ctx("agent-x").await;
    let tool = ChannelTool;

    tool.execute(serde_json::json!({
        "operation": "subscribe",
        "channel": "alerts"
    }), &ctx).await.unwrap();

    let result = tool.execute(serde_json::json!({
        "operation": "unsubscribe",
        "channel": "alerts"
    }), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("#alerts"));
}

#[tokio::test]
async fn test_channel_list_empty() {
    let (_shared, ctx) = make_ctx("lonely").await;
    let tool = ChannelTool;

    let result = tool.execute(serde_json::json!({
        "operation": "list"
    }), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("No channels"));
}
