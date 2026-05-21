use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_mcp::{HubMcpClient, IpcBudget, McpProvider, McpProxyClient};
use serde_json::{Value, json};

struct HangingHub;

#[async_trait]
impl HubMcpClient for HangingHub {
    async fn send_request(&self, _: &str, _: Value) -> Result<Value, String> {
        std::future::pending::<Result<Value, String>>().await
    }
}

#[tokio::test]
async fn forbidden_rejects_list_tools_without_ipc_wait() {
    let proxy = McpProxyClient::new(Arc::new(HangingHub));
    let start = std::time::Instant::now();
    let tools = proxy.list_tools(IpcBudget::Forbidden).await;
    let elapsed = start.elapsed();
    assert!(
        tools.is_empty(),
        "Forbidden must fall back to empty without waiting"
    );
    assert!(
        elapsed < Duration::from_millis(50),
        "Forbidden must return immediately, took {elapsed:?}"
    );
}

#[tokio::test]
async fn forbidden_rejects_call_tool_immediately() {
    let proxy = McpProxyClient::new(Arc::new(HangingHub));
    let start = std::time::Instant::now();
    let err = proxy
        .call_tool("s", "t", &json!({}), IpcBudget::Forbidden)
        .await
        .unwrap_err();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(50),
        "Forbidden must surface Err immediately, took {elapsed:?}"
    );
    let msg = format!("{err}");
    assert!(
        msg.contains("Forbidden") && msg.contains("critical path"),
        "error must name the Forbidden constraint, got: {msg}"
    );
}

#[tokio::test]
async fn forbidden_rejects_snapshot_without_ipc_wait() {
    let proxy = McpProxyClient::new(Arc::new(HangingHub));
    let start = std::time::Instant::now();
    let snaps = proxy.snapshot(IpcBudget::Forbidden).await;
    let elapsed = start.elapsed();
    assert!(
        snaps.is_empty(),
        "Forbidden must fall back to empty without waiting"
    );
    assert!(
        elapsed < Duration::from_millis(50),
        "Forbidden must return immediately, took {elapsed:?}"
    );
}
