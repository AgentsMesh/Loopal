use std::sync::Arc;

use loopal_hooks::HookRegistry;
use loopal_mcp::{McpManager, McpToolAdapter};
use loopal_provider::ProviderRegistry;
use loopal_tools::ToolRegistry;
use loopal_types::config::Settings;
use loopal_types::error::Result;
use loopal_types::hook::HookEvent;
use loopal_types::sandbox::ResolvedPolicy;
use loopal_types::tool::ToolDefinition;
use tokio::sync::RwLock;
use tracing::info;

use crate::provider_registry;

pub struct Kernel {
    tool_registry: ToolRegistry,
    provider_registry: ProviderRegistry,
    hook_registry: HookRegistry,
    mcp_manager: Arc<RwLock<McpManager>>,
    settings: Settings,
    sandbox_policy: Option<ResolvedPolicy>,
}

impl Kernel {
    pub fn new(settings: Settings) -> Result<Self> {
        // Initialize tool registry with builtins
        let mut tool_registry = ToolRegistry::new();
        loopal_tools::builtin::register_all(&mut tool_registry);

        // Initialize provider registry from config
        let mut provider_registry = ProviderRegistry::new();
        provider_registry::register_providers(&settings, &mut provider_registry);

        // Initialize hook registry
        let hook_registry = HookRegistry::new(settings.hooks.clone());

        // Initialize MCP manager (connections started separately via start_mcp)
        let mcp_manager = Arc::new(RwLock::new(McpManager::new()));

        info!("kernel initialized");

        Ok(Self {
            tool_registry,
            provider_registry,
            hook_registry,
            mcp_manager,
            settings,
            sandbox_policy: None,
        })
    }

    // --- Accessor methods ---

    /// Access the tool registry
    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Access the provider registry
    pub fn provider_registry(&self) -> &ProviderRegistry {
        &self.provider_registry
    }

    /// Register an additional tool into the tool registry (before wrapping in Arc).
    pub fn register_tool(&mut self, tool: Box<dyn loopal_types::tool::Tool>) {
        self.tool_registry.register(tool);
    }

    /// Register an additional provider (useful for testing with mock providers).
    pub fn register_provider(&mut self, provider: Arc<dyn loopal_types::provider::Provider>) {
        self.provider_registry.register(provider);
    }

    /// Access the hook registry
    pub fn hook_registry(&self) -> &HookRegistry {
        &self.hook_registry
    }

    /// Access the MCP manager
    pub fn mcp_manager(&self) -> &Arc<RwLock<McpManager>> {
        &self.mcp_manager
    }

    /// Access settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Initialize sandbox policy and wrap all registered tools with the decorator.
    pub fn init_sandbox(&mut self, cwd: &std::path::Path) {
        use loopal_types::sandbox::SandboxPolicy;
        if self.settings.sandbox.policy != SandboxPolicy::Disabled {
            let resolved = loopal_sandbox::resolve_policy(
                &self.settings.sandbox,
                cwd,
            );
            info!(
                policy = ?resolved.policy,
                writable_paths = resolved.writable_paths.len(),
                "sandbox initialized"
            );
            // Wrap all tools (builtins + MCP) with the sandbox decorator
            let policy = resolved.clone();
            self.tool_registry.wrap_all(move |inner| {
                Box::new(loopal_sandbox::SandboxedTool::new(inner, policy.clone()))
            });
            self.sandbox_policy = Some(resolved);
        }
    }

    /// Get the resolved sandbox policy, if sandboxing is enabled.
    pub fn sandbox_policy(&self) -> Option<&ResolvedPolicy> {
        self.sandbox_policy.as_ref()
    }

    // --- Convenience methods ---

    /// Get a tool by name from the registry
    pub fn get_tool(&self, name: &str) -> Option<&dyn loopal_types::tool::Tool> {
        self.tool_registry.get(name)
    }

    /// Get tool definitions for LLM
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_registry.to_definitions()
    }

    /// Resolve a provider for the given model
    pub fn resolve_provider(
        &self,
        model: &str,
    ) -> std::result::Result<
        Arc<dyn loopal_types::provider::Provider>,
        loopal_types::error::LoopalError,
    > {
        self.provider_registry.resolve(model)
    }

    /// Get hooks matching the given event and optional tool name
    pub fn get_hooks(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
    ) -> Vec<&loopal_types::hook::HookConfig> {
        self.hook_registry.match_hooks(event, tool_name)
    }

    /// Start all configured MCP servers and register their tools.
    pub async fn start_mcp(&mut self) -> Result<()> {
        if !self.settings.mcp_servers.is_empty() {
            let mut mgr = self.mcp_manager.write().await;
            mgr.start_all(&self.settings.mcp_servers).await?;
            info!(
                count = self.settings.mcp_servers.len(),
                "MCP servers started"
            );

            // Register MCP tools into the tool registry
            let tools_with_server = mgr.get_tools_with_server().await?;
            drop(mgr); // Release lock before registering

            for (server_name, tool_def) in tools_with_server {
                info!(tool = %tool_def.name, server = %server_name, "registering MCP tool");
                let adapter = McpToolAdapter::new(
                    tool_def,
                    server_name,
                    Arc::clone(&self.mcp_manager),
                );
                self.tool_registry.register(Box::new(adapter));
            }
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        // MCP connections will be dropped when McpManager is dropped.
        // Future: explicit graceful shutdown of MCP clients if needed.
        info!("kernel shutting down");
    }
}
