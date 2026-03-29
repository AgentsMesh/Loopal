//! ACP (Agent Client Protocol) server for IDE integration.
//!
//! Provides a JSON-RPC 2.0 interface over stdin/stdout, activated via `--acp`.
//! Connects to the Hub as a UI client via `UiSession`.

mod adapter;
pub mod jsonrpc;
mod translate;
pub mod types;

use tokio::io::BufReader;
use tracing::info;

use loopal_agent_hub::UiSession;

pub use crate::adapter::AcpAdapter;
use crate::jsonrpc::JsonRpcTransport;

/// Run ACP server over stdin/stdout, backed by a UiSession.
pub async fn run_acp(ui_session: UiSession) -> anyhow::Result<()> {
    info!("starting ACP server");
    let acp_out = std::sync::Arc::new(JsonRpcTransport::new());
    let adapter = AcpAdapter::new(ui_session, acp_out);
    let mut reader = BufReader::new(tokio::io::stdin());
    adapter.run(&mut reader).await
}
