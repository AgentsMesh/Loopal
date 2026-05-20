use std::sync::Arc;

use async_trait::async_trait;
use loopal_ipc::connection::Connection;
use loopal_mcp::HubMcpClient;
use serde_json::Value;

pub struct ConnectionMcpClient {
    connection: Arc<Connection>,
}

impl ConnectionMcpClient {
    pub fn new(connection: Arc<Connection>) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl HubMcpClient for ConnectionMcpClient {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.connection
            .send_request(method, params)
            .await
            .map_err(|e| e.to_string())
    }
}
