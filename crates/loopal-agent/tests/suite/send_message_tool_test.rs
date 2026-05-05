use std::sync::Arc;

use loopal_agent::shared::{AgentShared, SchedulerHandle};
use loopal_agent::task_store::TaskStore;
use loopal_agent::tools::collaboration::send_message::SendMessageTool;
use loopal_config::Settings;
use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_kernel::Kernel;
use loopal_protocol::Envelope;
use loopal_scheduler::CronScheduler;
use loopal_test_support::TestFixture;
use loopal_tool_api::{Tool, ToolContext};
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn make_ctx_with_hub_peer(fixture: &TestFixture) -> (ToolContext, Arc<Connection>) {
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let cwd = fixture
        .path()
        .canonicalize()
        .unwrap_or_else(|_| fixture.path().to_path_buf());
    let backend = loopal_backend::LocalBackend::new(
        cwd.clone(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    let (hub_conn, hub_peer) = loopal_ipc::duplex_pair();
    let hub_connection = Arc::new(Connection::new(hub_conn));
    let mut hub_conn_rx = hub_connection.start();
    tokio::spawn(async move { while hub_conn_rx.recv().await.is_some() {} });
    let hub_peer = Arc::new(Connection::new(hub_peer));

    let scheduler_handle =
        SchedulerHandle::new(Arc::new(CronScheduler::new()), CancellationToken::new());
    let shared = Arc::new(AgentShared {
        kernel,
        task_store: Arc::new(TaskStore::with_sessions_root(fixture.path().join("tasks"))),
        hub_connection,
        cwd,
        depth: 0,
        agent_name: "tester".into(),
        parent_event_tx: None,
        cancel_token: None,
        scheduler_handle,
        message_snapshot: Arc::new(std::sync::RwLock::new(Vec::new())),
    });
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared);
    (
        ToolContext::new(backend, "test-session").with_shared(shared_any),
        hub_peer,
    )
}

async fn intercept_routed_envelope(hub_peer: Arc<Connection>) -> Envelope {
    let mut rx = hub_peer.start();
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, method, params } = msg {
            assert_eq!(method, "hub/route");
            let env: Envelope = serde_json::from_value(params).unwrap();
            hub_peer.respond(id, json!({"ok": true})).await.unwrap();
            return env;
        }
    }
    panic!("no hub/route request received");
}

#[tokio::test]
async fn test_send_message_without_summary_omits_field() {
    let fixture = TestFixture::new();
    let (ctx, hub_peer) = make_ctx_with_hub_peer(&fixture);

    let tool_call = tokio::spawn(async move {
        SendMessageTool
            .execute(json!({"to": "main", "message": "hi"}), &ctx)
            .await
            .unwrap()
    });
    let env = intercept_routed_envelope(hub_peer).await;
    let result = tool_call.await.unwrap();

    assert!(!result.is_error);
    assert!(env.summary.is_none());
    assert_eq!(env.content.text, "hi");
}

#[tokio::test]
async fn test_send_message_with_summary_propagates_to_envelope() {
    let fixture = TestFixture::new();
    let (ctx, hub_peer) = make_ctx_with_hub_peer(&fixture);

    let tool_call = tokio::spawn(async move {
        SendMessageTool
            .execute(
                json!({"to": "main", "message": "long body", "summary": "ping"}),
                &ctx,
            )
            .await
            .unwrap()
    });
    let env = intercept_routed_envelope(hub_peer).await;
    let result = tool_call.await.unwrap();

    assert!(!result.is_error);
    assert_eq!(env.summary.as_deref(), Some("ping"));
    assert_eq!(env.content.text, "long body");
}

#[tokio::test]
async fn test_send_message_empty_summary_is_treated_as_none() {
    let fixture = TestFixture::new();
    let (ctx, hub_peer) = make_ctx_with_hub_peer(&fixture);

    let tool_call = tokio::spawn(async move {
        SendMessageTool
            .execute(json!({"to": "main", "message": "x", "summary": ""}), &ctx)
            .await
            .unwrap()
    });
    let env = intercept_routed_envelope(hub_peer).await;
    let _ = tool_call.await.unwrap();

    assert!(env.summary.is_none());
}

#[tokio::test]
async fn test_send_message_whitespace_only_summary_is_treated_as_none() {
    let fixture = TestFixture::new();
    let (ctx, hub_peer) = make_ctx_with_hub_peer(&fixture);

    let tool_call = tokio::spawn(async move {
        SendMessageTool
            .execute(
                json!({"to": "main", "message": "x", "summary": "   "}),
                &ctx,
            )
            .await
            .unwrap()
    });
    let env = intercept_routed_envelope(hub_peer).await;
    let _ = tool_call.await.unwrap();

    assert!(
        env.summary.is_none(),
        "whitespace-only summary must be normalized to None"
    );
}

#[tokio::test]
async fn test_send_message_missing_target_returns_invalid_input_error() {
    let fixture = TestFixture::new();
    let (ctx, _hub_peer) = make_ctx_with_hub_peer(&fixture);
    let result = SendMessageTool
        .execute(json!({"message": "no target"}), &ctx)
        .await;
    assert!(result.is_err());
}
