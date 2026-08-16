/// Transport factory functions for MCP server connections.
///
/// Creates transport and connects to MCP server in one step.
/// HTTP connections automatically fall back to OAuth if auth is required.
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use tokio::sync::Mutex;
use tracing::{info, warn};
use zeroize::Zeroizing;

use crate::client::McpClient;
use crate::contained_stdio_transport::ContainedStdioTransport;
use crate::handler::SamplingCallback;
use crate::handshake_transport::HandshakePolicy;
use crate::safe_diagnostics::{REDACTED_STDERR, connection_failed, endpoint_label};
use crate::stdio_command::stdio_command;

/// Connect to an MCP server over stdio (child process).
pub async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, Zeroizing<String>>,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
    stderr_tail: Option<Arc<Mutex<VecDeque<String>>>>,
) -> Result<McpClient, McpError> {
    connect_stdio_with_policy(
        command,
        args,
        env,
        timeout,
        sampling,
        stderr_tail,
        HandshakePolicy::Strip,
    )
    .await
}

pub(crate) async fn connect_stdio_with_policy(
    command: &str,
    args: &[String],
    env: &HashMap<String, Zeroizing<String>>,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
    stderr_tail: Option<Arc<Mutex<VecDeque<String>>>>,
    handshake_policy: HandshakePolicy,
) -> Result<McpClient, McpError> {
    info!(
        transport = "stdio",
        arg_count = args.len(),
        env_count = env.len(),
        "spawning MCP server"
    );

    let cmd = stdio_command(command, args, env);
    let (transport, stderr) = ContainedStdioTransport::spawn(cmd)
        .map_err(|_| connection_failed("MCP stdio spawn failed"))?;

    // Retain only the presence of stderr; server output may echo expanded secrets.
    if let Some(stderr) = stderr {
        spawn_stderr_drain(stderr, stderr_tail);
    }

    McpClient::connect_with_policy(transport, timeout, sampling, handshake_policy).await
}

/// Connect to an MCP server over Streamable HTTP.
///
/// If the initial connection fails with an auth error, automatically
/// falls back to OAuth browser-based authorization.
pub(crate) async fn connect_http(
    url: &str,
    client: crate::scoped_http_client::ScopedHttpClient,
    timeout: Duration,
    sampling: Option<Arc<dyn SamplingCallback>>,
    handshake_policy: HandshakePolicy,
) -> Result<McpClient, McpError> {
    use rmcp::transport::WorkerTransport;
    use rmcp::transport::streamable_http_client::{
        StreamableHttpClientTransportConfig, StreamableHttpClientWorker,
    };

    let endpoint = endpoint_label(url);
    info!(%endpoint, "connecting to MCP HTTP server");

    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let worker = StreamableHttpClientWorker::new(client, config);
    let transport = WorkerTransport::spawn(worker);

    match McpClient::connect_with_policy(transport, timeout, sampling.clone(), handshake_policy)
        .await
    {
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
    msg.contains("auth") || msg.contains("401")
}

fn spawn_stderr_drain(
    stderr: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    tail: Option<Arc<Mutex<VecDeque<String>>>>,
) {
    tokio::spawn(async move {
        drain_stderr(stderr, tail).await;
    });
}

/// Record bounded, redacted stderr presence for diagnostics.
async fn drain_stderr(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    stderr_tail: Option<Arc<Mutex<VecDeque<String>>>>,
) {
    use tokio::io::AsyncReadExt;
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

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
