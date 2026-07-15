/// Transport factory functions for MCP server connections.
///
/// Creates transport and connects to MCP server in one step.
/// HTTP connections automatically fall back to OAuth if auth is required.
use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use rmcp::transport::child_process::TokioChildProcess;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::client::McpClient;
use crate::handler::SamplingCallback;
use crate::safe_diagnostics::{REDACTED_STDERR, connection_failed, endpoint_label};
use crate::stdio_command::stdio_command;

/// Connect to an MCP server over stdio (child process).
pub async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
    stderr_tail: Option<Arc<Mutex<VecDeque<String>>>>,
) -> Result<McpClient, McpError> {
    info!(
        transport = "stdio",
        arg_count = args.len(),
        env_count = env.len(),
        "spawning MCP server"
    );

    let cmd = stdio_command(command, args, env);
    // reason: ensure MCP child is reaped when its parent (Hub or root agent)
    // exits. Without this, killing Hub leaves chrome-devtools-mcp / chrome
    // grandchildren running indefinitely. Set kill_on_drop so the Tokio child
    // handle's Drop sends SIGKILL.
    let (transport, stderr) = TokioChildProcess::builder(cmd)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| connection_failed("MCP stdio spawn failed"))?;

    // Retain only the presence of stderr; server output may echo expanded secrets.
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            drain_stderr(stderr, stderr_tail).await;
        });
    }

    McpClient::connect(transport, timeout, sampling).await
}

/// Connect to an MCP server over Streamable HTTP.
///
/// If the initial connection fails with an auth error, automatically
/// falls back to OAuth browser-based authorization.
pub async fn connect_http(
    url: &str,
    headers: &HashMap<String, String>,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
) -> Result<McpClient, McpError> {
    use rmcp::transport::WorkerTransport;
    use rmcp::transport::streamable_http_client::{
        StreamableHttpClientTransportConfig, StreamableHttpClientWorker,
    };

    let endpoint = endpoint_label(url);
    info!(%endpoint, header_count = headers.len(), "connecting to MCP HTTP server");

    let http_client = build_http_client(headers)?;
    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let worker = StreamableHttpClientWorker::new(http_client, config);
    let transport = WorkerTransport::spawn(worker);

    match McpClient::connect(transport, timeout, sampling.clone()).await {
        Ok(client) => Ok(client),
        Err(e) if is_auth_error(&e) => {
            warn!(%endpoint, "auth required, starting OAuth flow");
            crate::oauth::flow::connect_with_oauth(url, timeout, sampling).await
        }
        Err(_) => Err(connection_failed("MCP HTTP connection failed")),
    }
}

/// Check if an McpError indicates authentication is required.
fn is_auth_error(err: &McpError) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("auth") || msg.contains("401") || msg.contains("unauthorized")
}

/// Build a reqwest client with custom default headers and connection timeout.
fn build_http_client(headers: &HashMap<String, String>) -> Result<reqwest::Client, McpError> {
    let mut header_map = reqwest::header::HeaderMap::new();
    for (k, v) in headers {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| connection_failed("invalid MCP HTTP header name"))?;
        let value = reqwest::header::HeaderValue::from_str(v)
            .map_err(|_| connection_failed("invalid MCP HTTP header value"))?;
        header_map.insert(name, value);
    }

    reqwest::Client::builder()
        .default_headers(header_map)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| connection_failed("MCP HTTP client initialization failed"))
}

/// Record bounded, redacted stderr presence for diagnostics.
async fn drain_stderr(
    stderr: tokio::process::ChildStderr,
    stderr_tail: Option<Arc<Mutex<VecDeque<String>>>>,
) {
    use tokio::io::AsyncReadExt;
    let mut reader = stderr;
    let mut buffer = [0u8; 8192];
    let mut recorded = false;
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(length) => {
                if !recorded
                    && buffer[..length]
                        .iter()
                        .any(|byte| !byte.is_ascii_whitespace())
                {
                    recorded = true;
                    warn!(transport = "stdio", "{REDACTED_STDERR}");
                    if let Some(tail) = stderr_tail.as_ref() {
                        let mut t = tail.lock().await;
                        if t.len() == t.capacity().max(16) {
                            t.pop_front();
                        }
                        t.push_back(REDACTED_STDERR.to_string());
                    }
                }
            }
        }
    }
}
