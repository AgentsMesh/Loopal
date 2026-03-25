//! ACP (Agent Client Protocol) server for IDE integration.
//!
//! Provides a JSON-RPC 2.0 interface over stdin/stdout, activated via `--acp`.
//! Replaces the TUI with a machine-readable protocol suitable for Zed,
//! JetBrains, Neovim, and other editors.

mod frontend;
mod handler;
mod handler_prompt;
mod handler_session;
pub mod jsonrpc;
mod translate;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::info;

use loopal_config::ResolvedConfig;

pub use crate::handler::AcpHandler;
use crate::jsonrpc::{IncomingMessage, JsonRpcTransport, read_message};

/// Run Loopal as an ACP server (stdin/stdout JSON-RPC).
///
/// This is the main entry point when `--acp` is passed. It replaces the TUI
/// entirely — all interaction happens through the ACP protocol.
pub async fn run_acp(config: ResolvedConfig, cwd: PathBuf) -> anyhow::Result<()> {
    info!("starting ACP server");

    let transport = Arc::new(JsonRpcTransport::new());
    let handler = Arc::new(AcpHandler::new(transport.clone(), config, cwd));
    let mut reader = BufReader::new(tokio::io::stdin());

    run_acp_loop(&handler, &transport, &mut reader).await
}

/// Reader-agnostic ACP event loop.
///
/// Extracted from [`run_acp`] so that tests can substitute an in-memory
/// reader for stdin. The `handler` and `transport` are shared across the
/// loop and all spawned request handlers.
pub async fn run_acp_loop(
    handler: &Arc<AcpHandler>,
    transport: &Arc<JsonRpcTransport>,
    reader: &mut (impl AsyncBufReadExt + Unpin),
) -> anyhow::Result<()> {
    loop {
        match read_message(reader).await {
            Some(IncomingMessage::Request { id, method, params }) => {
                let h = handler.clone();
                tokio::spawn(async move {
                    h.dispatch(id, &method, params).await;
                });
            }
            Some(IncomingMessage::Response { id, result, error }) => {
                let value = if let Some(r) = result {
                    r
                } else if let Some(e) = error {
                    serde_json::to_value(e).unwrap_or_default()
                } else {
                    serde_json::Value::Null
                };
                transport.route_response(id, value).await;
            }
            Some(IncomingMessage::Notification { method, .. }) => {
                info!(method = %method, "received notification (ignored)");
            }
            None => {
                info!("reader closed, shutting down ACP server");
                break;
            }
        }
    }

    Ok(())
}
