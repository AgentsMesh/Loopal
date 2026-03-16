use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use loopagent_types::error::McpError;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};
use tracing::{debug, info, warn};

type PendingMap = HashMap<u64, oneshot::Sender<Result<Value, McpError>>>;

/// JSON-RPC stdio client for a single MCP server.
pub struct McpClient {
    writer: Arc<Mutex<tokio::process::ChildStdin>>,
    pending: Arc<Mutex<PendingMap>>,
    next_id: AtomicU64,
    _child: Child,
}

impl McpClient {
    /// Spawn a child process and start reading its stdout.
    pub async fn start(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| McpError::ConnectionFailed(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::ConnectionFailed("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::ConnectionFailed("no stdout".into()))?;

        let writer = Arc::new(Mutex::new(stdin));
        let pending: Arc<Mutex<PendingMap>> = Arc::new(Mutex::new(HashMap::new()));

        info!(command = %command, args = ?args, "MCP process started");

        // Spawn reader task
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        info!("MCP process stdout EOF");
                        break;
                    }
                    Ok(_) => {
                        if let Ok(msg) = serde_json::from_str::<Value>(&line)
                            && let Some(id) = msg.get("id").and_then(|v| v.as_u64()) {
                                let mut map = pending_clone.lock().await;
                                if let Some(tx) = map.remove(&id) {
                                    if let Some(err) = msg.get("error") {
                                        let _ = tx.send(Err(McpError::Protocol(
                                            err.to_string(),
                                        )));
                                    } else {
                                        let result =
                                            msg.get("result").cloned().unwrap_or(Value::Null);
                                        let _ = tx.send(Ok(result));
                                    }
                                }
                            }
                    }
                    Err(e) => {
                        warn!(error = %e, "MCP process read error");
                        break;
                    }
                }
            }
        });

        let client = Self {
            writer,
            pending,
            next_id: AtomicU64::new(1),
            _child: child,
        };

        // Perform initialize handshake
        client.initialize().await?;

        Ok(client)
    }

    async fn initialize(&self) -> Result<(), McpError> {
        let _result = self
            .send_request(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "loopagent",
                        "version": "0.1.0"
                    }
                }),
            )
            .await?;

        // Send initialized notification (no id, no response expected)
        self.send_notification("notifications/initialized", serde_json::json!({}))
            .await?;

        Ok(())
    }

    /// Send a JSON-RPC request and wait for the response.
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let start = std::time::Instant::now();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        let mut data = serde_json::to_vec(&request)
            .map_err(|e| McpError::Protocol(e.to_string()))?;
        data.push(b'\n');

        {
            let mut writer = self.writer.lock().await;
            writer
                .write_all(&data)
                .await
                .map_err(|e| McpError::ConnectionFailed(e.to_string()))?;
            writer
                .flush()
                .await
                .map_err(|e| McpError::ConnectionFailed(e.to_string()))?;
        }

        debug!(id, method, "sent request");

        let result = rx.await
            .map_err(|_| McpError::ConnectionFailed("response channel closed".into()))?;
        let duration = start.elapsed();
        match &result {
            Ok(_) => info!(id, method, duration_ms = duration.as_millis() as u64, "MCP response"),
            Err(e) => warn!(id, method, duration_ms = duration.as_millis() as u64, error = %e, "MCP error"),
        }
        result
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, method: &str, params: Value) -> Result<(), McpError> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let mut data = serde_json::to_vec(&notification)
            .map_err(|e| McpError::Protocol(e.to_string()))?;
        data.push(b'\n');

        let mut writer = self.writer.lock().await;
        writer
            .write_all(&data)
            .await
            .map_err(|e| McpError::ConnectionFailed(e.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|e| McpError::ConnectionFailed(e.to_string()))?;

        Ok(())
    }
}
