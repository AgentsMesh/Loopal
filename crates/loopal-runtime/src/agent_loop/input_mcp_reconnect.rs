use loopal_error::Result;
use tracing::{error, info};

use super::input_control::ControlOutcome;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn handle_mcp_reconnect(&mut self, server: String) -> Result<ControlOutcome> {
        info!(server = %server, "reconnecting MCP server");
        let provider = self.params.deps.kernel.mcp_provider();
        let rejection = match provider
            .reconnect(&server, loopal_mcp::HUB_RPC_BUDGET)
            .await
        {
            Ok(()) => {
                self.params
                    .deps
                    .kernel
                    .register_all_settled_mcp_tools()
                    .await;
                None
            }
            Err(error) => {
                error!(server = %server, %error, "MCP reconnect failed");
                Some(format!("MCP reconnect failed for {server}"))
            }
        };
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(loopal_protocol::AgentEventPayload::McpStatusReport { servers: snapshots })
            .await?;
        Ok(rejection.map_or_else(ControlOutcome::applied, ControlOutcome::rejected))
    }
}
