//! Hermetic stdio MCP server for e2e tests: newline JSON-RPC, one `mcp_echo`
//! tool. Spawned by the real Hub (LocalMcpProvider) — keep it dependency-light
//! and instant to start.

use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let id = req["id"].clone();
        if id.is_null() {
            continue;
        }
        let result = match req["method"].as_str().unwrap_or_default() {
            "initialize" => serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "loopal-e2e-mock-mcp", "version": "1.0.0"}
            }),
            "tools/list" => serde_json::json!({
                "tools": [{
                    "name": "mcp_echo",
                    "description": "Echo back the given text.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"]
                    }
                }]
            }),
            "tools/call" => {
                let text = req["params"]["arguments"]["text"]
                    .as_str()
                    .unwrap_or_default();
                serde_json::json!({
                    "content": [{"type": "text", "text": format!("mcp_echo: {text}")}],
                    "isError": false
                })
            }
            _ => serde_json::json!({}),
        };
        let resp = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
        let _ = writeln!(out, "{resp}");
        let _ = out.flush();
    }
}
