use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use indexmap::IndexMap;
use loopal_config::McpServerConfig;
use loopal_error::McpError;
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;
use tokio::sync::{Notify, RwLock};
use tokio::time::Instant;

use crate::manager::McpManager;
use crate::manager_query::McpConnectionSnapshot;
use crate::provider::McpProvider;

/// Running-task counter: `wait()` returns true exactly when every spawn that
/// was outstanding at any point during the wait has completed. Fast tasks
/// cannot prematurely declare "settled" on behalf of still-running peers.
struct SettleSignal {
    running: AtomicU64,
    notify: Notify,
}

impl SettleSignal {
    fn new() -> Self {
        Self {
            running: AtomicU64::new(0),
            notify: Notify::new(),
        }
    }

    fn mark_running(&self) {
        self.running.fetch_add(1, Ordering::AcqRel);
    }

    fn mark_settled(&self) {
        if self.running.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.notify.notify_waiters();
        }
    }

    async fn wait(&self, timeout: Duration) -> bool {
        if self.running.load(Ordering::Acquire) == 0 {
            return true;
        }
        let deadline = Instant::now() + timeout;
        loop {
            if self.running.load(Ordering::Acquire) == 0 {
                return true;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            if tokio::time::timeout(remaining, self.notify.notified())
                .await
                .is_err()
            {
                return false;
            }
        }
    }

    /// Block (no timeout) until every outstanding spawn finishes. Used by
    /// late-registration listeners that must react to MCP servers coming
    /// online *after* the bounded-wait budget elapsed.
    async fn wait_forever(&self) {
        loop {
            if self.running.load(Ordering::Acquire) == 0 {
                return;
            }
            self.notify.notified().await;
        }
    }
}

pub struct LocalMcpProvider {
    manager: Arc<RwLock<McpManager>>,
    signal: Arc<SettleSignal>,
    /// Bumped each time a background spawn finishes. Lets the runtime
    /// subscribe and push an updated `McpStatusReport` to the TUI so users
    /// see "connecting → connected" transitions without polling.
    settle_revision: tokio::sync::watch::Sender<u64>,
}

impl LocalMcpProvider {
    pub fn new(manager: Arc<RwLock<McpManager>>) -> Self {
        let (settle_revision, _) = tokio::sync::watch::channel(0u64);
        Self {
            manager,
            signal: Arc::new(SettleSignal::new()),
            settle_revision,
        }
    }

    pub fn manager(&self) -> Arc<RwLock<McpManager>> {
        self.manager.clone()
    }

    pub fn subscribe_settle_events(&self) -> tokio::sync::watch::Receiver<u64> {
        self.settle_revision.subscribe()
    }

    pub fn spawn_background(&self, configs: IndexMap<String, McpServerConfig>) {
        if configs.is_empty() {
            return;
        }
        let manager = self.manager.clone();
        let signal = self.signal.clone();
        let revision = self.settle_revision.clone();
        signal.mark_running();
        tokio::spawn(async move {
            // reason: connect() can take 30s+ for slow stdio servers. If we
            // held manager.write() across this await, every reader (snapshot,
            // list_tools, finalize_mcp_tools' metadata pull) would block,
            // defeating the entire bounded-wait design. So: snapshot
            // sampling/secrets behind a read lock (instant), connect with
            // NO lock held, then take a brief write lock to commit results.
            let prepared = {
                let mgr = manager.read().await;
                mgr.prepare_connections(&configs).await
            };
            let results = crate::manager::connect_all(prepared).await;
            {
                let mut mgr = manager.write().await;
                if let Err(e) = mgr.absorb_connections(results) {
                    tracing::warn!(error = %e, "MCP background spawn finished with error");
                }
            }
            signal.mark_settled();
            revision.send_modify(|v| *v = v.wrapping_add(1));
        });
    }

    pub async fn wait_until_settled(&self, timeout: Duration) -> bool {
        self.signal.wait(timeout).await
    }

    /// Block until every background spawn has finished. Listener tasks use
    /// this to register tools that only became available after the bounded
    /// `wait_until_settled` budget elapsed.
    pub async fn await_all_settled(&self) {
        self.signal.wait_forever().await;
    }

    pub async fn try_reconnect(&self, server: &str) -> bool {
        let mut mgr = self.manager.write().await;
        if let Err(e) = mgr.restart_connection(server).await {
            tracing::warn!(server, error = %e, "MCP restart_connection failed");
            return false;
        }
        true
    }
}

#[async_trait]
impl McpProvider for LocalMcpProvider {
    async fn list_tools(&self) -> Vec<(String, ToolDefinition)> {
        self.manager.read().await.get_tools_with_server()
    }

    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
    ) -> Result<CallToolResult, McpError> {
        let first = self.manager.read().await.call_tool(server, tool, args).await;
        match first {
            Err(McpError::TransportClosed(_)) => {
                tracing::warn!(server, tool, "MCP transport closed, attempting reconnect");
                let reconnected = self.try_reconnect(server).await;
                if !reconnected {
                    return first;
                }
                self.manager.read().await.call_tool(server, tool, args).await
            }
            other => other,
        }
    }

    async fn snapshot(&self) -> Vec<McpConnectionSnapshot> {
        self.manager.read().await.collect_snapshots()
    }
}


