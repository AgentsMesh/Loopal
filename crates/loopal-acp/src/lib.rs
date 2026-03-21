//! ACP (Agent Client Protocol) server for IDE integration.
//!
//! Provides a JSON-RPC 2.0 interface over stdin/stdout, activated via `--acp`.
//! Replaces the TUI with a machine-readable protocol suitable for Zed,
//! JetBrains, Neovim, and other editors.

mod frontend;
mod handler;
mod handler_prompt;
mod handler_session;
mod jsonrpc;
mod translate;
mod types;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::BufReader;
use tracing::info;

use loopal_config::Settings;

use crate::handler::AcpHandler;
use crate::jsonrpc::{IncomingMessage, JsonRpcTransport, read_message};

/// Run Loopal as an ACP server (stdin/stdout JSON-RPC).
///
/// This is the main entry point when `--acp` is passed. It replaces the TUI
/// entirely — all interaction happens through the ACP protocol.
pub async fn run_acp(settings: Settings, cwd: PathBuf) -> anyhow::Result<()> {
    info!("starting ACP server");

    let transport = Arc::new(JsonRpcTransport::new());
    let handler = Arc::new(AcpHandler::new(transport.clone(), settings, cwd));
    let mut reader = BufReader::new(tokio::io::stdin());

    loop {
        match read_message(&mut reader).await {
            Some(IncomingMessage::Request { id, method, params }) => {
                let h = handler.clone();
                tokio::spawn(async move {
                    h.dispatch(id, &method, params).await;
                });
            }
            Some(IncomingMessage::Response { id, result, error }) => {
                // Route response to a pending outbound request
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
                // EOF — client disconnected
                info!("stdin closed, shutting down ACP server");
                break;
            }
        }
    }

    Ok(())
}
