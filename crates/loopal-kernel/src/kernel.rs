use std::sync::Arc;

use loopal_config::HookEvent;
use loopal_config::Settings;
use loopal_error::Result;
use loopal_hooks::HookRegistry;
use loopal_mcp::{McpManager, McpToolAdapter};
use loopal_provider::ProviderRegistry;
use loopal_tool_api::ToolDefinition;
use loopal_tools::ToolRegistry;
use tokio::sync::RwLock;
use tracing::info;

use crate::provider_registry;

pub struct Kernel {
    tool_registry: ToolRegistry,
    provider_registry: ProviderRegistry,
    hook_registry: HookRegistry,
    mcp_manager: Arc<RwLock<McpManager>>,
    settings: Settings,
}

impl Kernel {
    pub fn new(settings: Settings) -> Result<Self> {
        let mut tool_registry = ToolRegistry::new();
        loopal_tools::builtin::register_all(&mut tool_registry);

        let mut provider_registry = ProviderRegistry::new();
        provider_registry::register_providers(&settings, &mut provider_registry);

        let hook_registry = HookRegistry::new(settings.hooks.clone());
        let mcp_manager = Arc::new(RwLock::new(McpManager::new()));

        info!("kernel initialized");

        Ok(Self {
            tool_registry,
            provider_registry,
            hook_registry,
            mcp_manager,
            settings,
        })
    }

    // --- Mutation methods (pre-Arc phase only) ---

    /// Register an additional tool (before wrapping in Arc).
    pub fn register_tool(&mut self, tool: Box<dyn loopal_tool_api::Tool>) {
        self.tool_registry.register(tool);
    }

    /// Register an additional provider (useful for testing with mock providers).
    pub fn register_provider(&mut self, provider: Arc<dyn loopal_provider_api::Provider>) {
        self.provider_registry.register(provider);
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

            let tools_with_server = mgr.get_tools_with_server().await?;
            drop(mgr);

            for (server_name, tool_def) in tools_with_server {
                info!(tool = %tool_def.name, server = %server_name, "registering MCP tool");
                let adapter =
                    McpToolAdapter::new(tool_def, server_name, Arc::clone(&self.mcp_manager));
                self.tool_registry.register(Box::new(adapter));
            }
        }
        Ok(())
    }

    // --- Query methods (post-Arc, immutable) ---

    /// Create a `LocalBackend` for the given working directory.
    ///
    /// Resolves the sandbox policy (if enabled) and bundles it with default
    /// resource limits. The returned `Arc` is injected into `ToolContext.backend`.
    pub fn create_backend(&self, cwd: &std::path::Path) -> Arc<dyn loopal_tool_api::Backend> {
        use loopal_config::SandboxPolicy;
        let policy = if self.settings.sandbox.policy != SandboxPolicy::Disabled {
            Some(loopal_sandbox::resolve_policy(&self.settings.sandbox, cwd))
        } else {
            None
        };
        loopal_backend::LocalBackend::new(
            cwd.to_path_buf(),
            policy,
            loopal_backend::ResourceLimits::default(),
        )
    }

    /// Access settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Get a tool by name from the registry.
    pub fn get_tool(&self, name: &str) -> Option<&dyn loopal_tool_api::Tool> {
        self.tool_registry.get(name)
    }

    /// Get tool definitions for LLM.
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_registry.to_definitions()
    }

    /// Resolve a provider for the given model.
    pub fn resolve_provider(
        &self,
        model: &str,
    ) -> std::result::Result<Arc<dyn loopal_provider_api::Provider>, loopal_error::LoopalError>
    {
        self.provider_registry.resolve(model)
    }

    /// Get hooks matching the given event and optional tool name.
    pub fn get_hooks(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
    ) -> Vec<&loopal_config::HookConfig> {
        self.hook_registry.match_hooks(event, tool_name)
    }
}
