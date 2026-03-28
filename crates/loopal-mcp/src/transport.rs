/// Transport factory functions for MCP server connections.
///
/// Creates transport and connects to MCP server in one step.
/// HTTP connections automatically fall back to OAuth if auth is required.
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use rmcp::transport::child_process::TokioChildProcess;
use tracing::{info, warn};

use crate::client::McpClient;
use crate::handler::SamplingCallback;

/// Connect to an MCP server over stdio (child process).
pub async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
) -> Result<McpClient, McpError> {
    info!(command, ?args, "spawning MCP stdio server");

    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }

    let (transport, stderr) = TokioChildProcess::builder(cmd)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| McpError::ConnectionFailed(format!("{command}: {e}")))?;

    // Drain child stderr to tracing so MCP server errors are visible in logs.
    if let Some(stderr) = stderr {
        let cmd_name = command.to_string();
        tokio::spawn(async move {
            drain_stderr(stderr, &cmd_name).await;
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

    info!(
        url,
        header_count = headers.len(),
        "connecting to MCP HTTP server"
    );

    let http_client = build_http_client(headers)?;
    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let worker = StreamableHttpClientWorker::new(http_client, config);
    let transport = WorkerTransport::spawn(worker);

    match McpClient::connect(transport, timeout, sampling.clone()).await {
        Ok(client) => Ok(client),
        Err(e) if is_auth_error(&e) => {
            warn!(url, "auth required, starting OAuth flow");
            crate::oauth::flow::connect_with_oauth(url, timeout, sampling).await
        }
        Err(e) => Err(e),
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
            .map_err(|e| McpError::ConnectionFailed(format!("invalid header '{k}': {e}")))?;
        let value = reqwest::header::HeaderValue::from_str(v)
            .map_err(|e| McpError::ConnectionFailed(format!("invalid header value: {e}")))?;
        header_map.insert(name, value);
    }

    reqwest::Client::builder()
        .default_headers(header_map)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| McpError::ConnectionFailed(format!("HTTP client: {e}")))
}

/// Forward child process stderr lines to tracing as warnings.
async fn drain_stderr(stderr: tokio::process::ChildStderr, server: &str) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    warn!(server, "MCP stderr: {trimmed}");
                }
            }
        }
    }
}
