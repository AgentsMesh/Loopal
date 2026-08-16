use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use indexmap::IndexMap;
use loopal_config::McpServerConfig;
use loopal_error::McpError;
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::manager::McpManager;
use crate::manager_query::McpConnectionSnapshot;
use crate::provider::McpProvider;
use crate::settle_signal::SettleSignal;

pub struct LocalMcpProvider {
    manager: Arc<RwLock<McpManager>>,
    signal: Arc<SettleSignal>,
    // Bumped each time a background spawn finishes; runtime subscribes and
    // pushes an updated McpStatusReport so users see connecting → connected
    // transitions without polling.
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
            // connect() can take 30s+ for slow stdio servers. Holding
            // manager.write() across the await would block every reader
            // (snapshot, list_tools, finalize_mcp_tools). Snapshot inputs
            // under read lock (instant), connect with NO lock, then take
            // a brief write lock to commit.
            let prepared = {
                let mgr = manager.read().await;
                mgr.prepare_connections(&configs).await
            };
            let results = crate::manager::connect_all(prepared).await;
            {
                let mut mgr = manager.write().await;
                if mgr.absorb_connections(results).is_err() {
                    tracing::warn!("MCP background spawn finished with error");
                }
            }
            signal.mark_settled();
            revision.send_modify(|v| *v = v.wrapping_add(1));
        });
    }

    pub async fn wait_until_settled(&self, timeout: Duration) -> bool {
        self.signal.wait(timeout).await
    }

    // Blocks until every background spawn has finished. Listener tasks use
    // this to register tools that became available after the bounded
    // `wait_until_settled` budget elapsed.
    pub async fn await_all_settled(&self) {
        self.signal.wait_forever().await;
    }

    pub async fn owns_server(&self, server: &str) -> bool {
        self.manager.read().await.connections.contains_key(server)
    }

    // O(1) check whether `server` has settled tools — equivalent to
    // `list_tools().iter().any(...)` but without iterating the full list.
    // provider_for_call dispatches on every tool invocation, so cheap matters.
    pub async fn has_server(&self, server: &str) -> bool {
        self.manager
            .read()
            .await
            .connections
            .get(server)
            .map(|conn| !conn.cached_tools.is_empty())
            .unwrap_or(false)
    }
}

#[async_trait]
impl McpProvider for LocalMcpProvider {
    async fn list_tools(&self, _budget: loopal_ipc::IpcBudget) -> Vec<(String, ToolDefinition)> {
        self.manager.read().await.get_tools_with_server()
    }

    async fn reconnect(
        &self,
        server: &str,
        _budget: loopal_ipc::IpcBudget,
    ) -> Result<(), McpError> {
        self.try_reconnect(server)
            .await
            .then_some(())
            .ok_or_else(|| McpError::ConnectionFailed("MCP reconnect failed".into()))
    }

    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
        _budget: loopal_ipc::IpcBudget,
    ) -> Result<CallToolResult, McpError> {
        let (failed_generation, first) = {
            let manager = self.manager.read().await;
            let generation = manager.connection_generation(server);
            let result = manager.call_tool(server, tool, args).await;
            (generation, result)
        };
        let transport_closed = matches!(&first, Err(McpError::TransportClosed(_)))
            || self
                .manager
                .read()
                .await
                .connections
                .get(server)
                .and_then(|connection| connection.client())
                .is_none_or(|client| client.is_closed());
        if first.is_err() && transport_closed {
            tracing::warn!(server, tool, "MCP transport closed, attempting reconnect");
            let reconnected = match failed_generation {
                Some(generation) => self.try_reconnect_after_failure(server, generation).await,
                None => self.try_reconnect(server).await,
            };
            if reconnected {
                return self
                    .manager
                    .read()
                    .await
                    .call_tool(server, tool, args)
                    .await;
            }
        }
        first
    }

    async fn snapshot(&self, _budget: loopal_ipc::IpcBudget) -> Vec<McpConnectionSnapshot> {
        self.manager.read().await.collect_snapshots()
    }
}

#[path = "local_provider_reconnect.rs"]
mod reconnect;

#[cfg(test)]
#[path = "local_provider_tests.rs"]
mod tests;
