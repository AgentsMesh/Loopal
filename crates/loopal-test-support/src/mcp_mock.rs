//! In-process mock MCP server for integration tests.

use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

/// A mock tool that returns a fixed response.
pub struct MockMcpTool {
    pub name: String,
    pub description: String,
    pub response: Value,
}

/// In-process MCP server speaking JSON-RPC over duplex streams.
pub struct MockMcpServer {
    tools: Vec<MockMcpTool>,
}

impl MockMcpServer {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn add_tool(mut self, name: &str, description: &str, response: Value) -> Self {
        self.tools.push(MockMcpTool {
            name: name.to_string(),
            description: description.to_string(),
            response,
        });
        self
    }

    /// Start the mock server. Returns (client_read, client_write) for the MCP client.
    pub fn start(self) -> (DuplexStream, DuplexStream) {
        let (client_write, server_read) = tokio::io::duplex(8192);
        let (server_write, client_read) = tokio::io::duplex(8192);

        let tools = Arc::new(self.tools);
        tokio::spawn(async move {
            run_server_loop(server_read, server_write, tools).await;
        });

        (client_read, client_write)
    }
}

impl Default for MockMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

async fn run_server_loop(
    server_read: DuplexStream,
    mut server_write: DuplexStream,
    tools: Arc<Vec<MockMcpTool>>,
) {
    let mut reader = BufReader::new(server_read);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: Value = match serde_json::from_str(line.trim()) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let id = msg.get("id").cloned();
                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let response = dispatch(method, &msg, &tools);
                let Some(response) = response else { continue };
                if let Some(id) = id {
                    let resp = json!({"jsonrpc": "2.0", "id": id, "result": response});
                    let mut bytes = serde_json::to_vec(&resp).unwrap();
                    bytes.push(b'\n');
                    let _ = server_write.write_all(&bytes).await;
                    let _ = server_write.flush().await;
                }
            }
        }
    }
}

fn dispatch(method: &str, msg: &Value, tools: &[MockMcpTool]) -> Option<Value> {
    match method {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "mock", "version": "0.1"}
        })),
        "notifications/initialized" => None,
        "tools/list" => {
            let list: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": {"type": "object", "properties": {}}
                    })
                })
                .collect();
            Some(json!({"tools": list}))
        }
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            match tools.iter().find(|t| t.name == name) {
                Some(t) => {
                    Some(json!({"content": [{"type": "text", "text": t.response.to_string()}]}))
                }
                None => Some(
                    json!({"content": [{"type": "text", "text": "unknown tool"}], "isError": true}),
                ),
            }
        }
        _ => Some(json!({"error": {"code": -32601, "message": "unknown method"}})),
    }
}
