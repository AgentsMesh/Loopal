use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct GatedMcpServer {
    pub url: String,
    enabled: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}

impl GatedMcpServer {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let enabled = Arc::new(AtomicBool::new(false));
        let gate = enabled.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let request = read_request(&mut socket).await;
                let response = if gate.load(Ordering::SeqCst) {
                    enabled_response(&request)
                } else {
                    response("503 Service Unavailable", "")
                };
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        Self {
            url: format!("http://{address}/mcp"),
            enabled,
            task,
        }
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }
}

impl Drop for GatedMcpServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn enabled_response(request: &str) -> String {
    let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let Some(id) = value.get("id").cloned() else {
        return response("202 Accepted", "");
    };
    let result = match value["method"].as_str().unwrap_or_default() {
        "initialize" => serde_json::json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "reconnect-fixture", "version": "1"}
        }),
        "tools/list" => serde_json::json!({"tools": [{
            "name": "mcp_echo",
            "description": "Echo text after reconnect.",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"]
            }
        }]}),
        "tools/call" => {
            let text = value["params"]["arguments"]["text"]
                .as_str()
                .unwrap_or_default();
            serde_json::json!({
                "content": [{"type": "text", "text": format!("mcp_echo: {text}")}],
                "isError": false
            })
        }
        _ => serde_json::json!({}),
    };
    response(
        "200 OK",
        &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
    )
}

fn response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let count = socket.read(&mut buffer).await.unwrap();
        if count == 0 {
            return String::from_utf8_lossy(&bytes).into_owned();
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header = String::from_utf8_lossy(&bytes[..header_end]);
    let length = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")?
                .parse::<usize>()
                .ok()
        })
        .unwrap_or_default();
    while bytes.len() < header_end + length {
        let count = socket.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
