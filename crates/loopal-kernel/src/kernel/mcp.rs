//! MCP-related methods on Kernel (dynamic tool registration, accessors).

use std::sync::Arc;

use loopal_mcp::McpToolAdapter;
use loopal_mcp::types::{McpPrompt, McpResource};
use loopal_tool_api::ToolDefinition;
use tracing::{info, warn};

use super::Kernel;

impl Kernel {
    /// MCP server instructions cached from the initialize handshake.
    pub fn mcp_instructions(&self) -> &[(String, String)] {
        &self.mcp_instructions
    }

    /// MCP resources cached at startup.
    pub fn mcp_resources(&self) -> &[(String, McpResource)] {
        &self.mcp_resources
    }

    /// MCP prompts cached at startup.
    pub fn mcp_prompts(&self) -> &[(String, McpPrompt)] {
        &self.mcp_prompts
    }

    /// Register tools from a (re)connected MCP server into the ToolRegistry.
    ///
    /// Called after `restart_connection()` succeeds so that tools from a
    /// previously-failed server become visible to the LLM.
    pub async fn register_mcp_tools_for_server(&self, server: &str) {
        let new_tools: Vec<ToolDefinition> = {
            let mgr = self.mcp_manager.read().await;
            mgr.get_tools_for_server(server)
        };
        for tool_def in new_tools {
            if self.tool_registry.get(&tool_def.name).is_some() {
                warn!(
                    tool = %tool_def.name, server = %server,
                    "MCP tool conflicts with existing tool, skipping"
                );
                continue;
            }
            info!(tool = %tool_def.name, server = %server, "dynamically registering MCP tool");
            let adapter =
                McpToolAdapter::new(tool_def, server.to_string(), Arc::clone(&self.mcp_manager));
            self.tool_registry.register(Box::new(adapter));
        }
    }

    /// Remove tools by name from the ToolRegistry.
    pub fn unregister_tools(&self, names: &[String]) {
        for name in names {
            info!(tool = %name, "unregistering MCP tool");
            self.tool_registry.unregister(name);
        }
    }
}
