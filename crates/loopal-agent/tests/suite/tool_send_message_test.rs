//! End-to-end integration tests for SendMessage and Channel tools
//! routing through MessageRouter instead of the old MessageBus/ChannelHub.

use std::sync::Arc;

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_agent::tools::send_message::SendMessageTool;
use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_protocol::AgentEvent;
use loopal_protocol::Envelope;
use loopal_tool_api::{Tool, ToolContext};
use tokio::sync::{Mutex, mpsc};

async fn make_shared_and_ctx() -> (Arc<AgentShared>, ToolContext, mpsc::Receiver<Envelope>) {
    let (event_tx, _) = mpsc::channel::<AgentEvent>(64);
    let router = Arc::new(MessageRouter::new(event_tx));

    // Register a target mailbox
    let (target_tx, target_rx) = mpsc::channel::<Envelope>(16);
    router.register("worker", target_tx).await.unwrap();

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let tmp = std::env::temp_dir().join(format!("la_tool_test_{}", std::process::id()));
    let shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        registry: Arc::new(Mutex::new(AgentRegistry::new())),
        task_store: Arc::new(TaskStore::new(tmp)),
        router,
        cwd: std::env::temp_dir(),
        depth: 0,
        max_depth: 3,
        agent_name: "main".to_string(),
        parent_event_tx: None,
        cancel_token: None,
        worktree_state: Default::default(),
    });

    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared.clone());
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    let ctx = ToolContext {
        backend,
        session_id: "test".to_string(),
        shared: Some(shared_any),
        pending_cwd_switch: Default::default(),
        memory_channel: None,
    };

    (shared, ctx, target_rx)
}

#[tokio::test]
async fn test_send_message_routes_via_router() {
    let (_shared, ctx, mut target_rx) = make_shared_and_ctx().await;
    let tool = SendMessageTool;

    let result = tool
        .execute(
            serde_json::json!({
                "type": "message",
                "recipient": "worker",
                "content": "hello from router"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("sent to 'worker'"));

    // Verify envelope was delivered to the target mailbox
    let envelope = target_rx.recv().await.expect("should receive envelope");
    assert_eq!(envelope.content.text, "hello from router");
    assert_eq!(envelope.target, "worker");
}

#[tokio::test]
async fn test_send_message_to_unknown_agent() {
    let (_shared, ctx, _) = make_shared_and_ctx().await;
    let tool = SendMessageTool;

    let result = tool
        .execute(
            serde_json::json!({
                "type": "message",
                "recipient": "nonexistent",
                "content": "hello"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("nonexistent"));
}

#[tokio::test]
async fn test_send_message_broadcast_via_router() {
    let (shared, ctx, mut target_rx) = make_shared_and_ctx().await;

    // Register a second mailbox
    let (tx2, mut rx2) = mpsc::channel::<Envelope>(16);
    shared.router.register("helper", tx2).await.unwrap();

    let tool = SendMessageTool;
    let result = tool
        .execute(
            serde_json::json!({
                "type": "broadcast",
                "content": "attention everyone"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("agents"));

    // Both agents should receive (main excluded as sender)
    let e1 = target_rx.recv().await.expect("worker should receive");
    assert_eq!(e1.content.text, "attention everyone");
    let e2 = rx2.recv().await.expect("helper should receive");
    assert_eq!(e2.content.text, "attention everyone");
}
