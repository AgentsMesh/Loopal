mod mcp;

use std::sync::Arc;

use loopal_config::HookEvent;
use loopal_config::Settings;
use loopal_error::Result;
use loopal_hooks::{HookRegistry, HookService};
use loopal_mcp::types::{McpPrompt, McpResource};
use loopal_mcp::{McpManager, McpToolAdapter};
use loopal_provider::ProviderRegistry;
use loopal_tool_api::ToolDefinition;
use loopal_tools::ToolRegistry;
use loopal_vault_api::Vault;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::hook_factory::DefaultExecutorFactory;
use crate::provider_registry;

use loopal_tool_background::BackgroundTaskStore;

use crate::bg_gc::spawn_bg_gc_tick;

pub struct Kernel {
    pub(super) tool_registry: ToolRegistry,
    provider_registry: ProviderRegistry,
    hook_service: HookService,
    pub(super) mcp_manager: Arc<RwLock<McpManager>>,
    pub(super) mcp_instructions: Vec<(String, String)>,
    pub(super) mcp_resources: Vec<(String, McpResource)>,
    pub(super) mcp_prompts: Vec<(String, McpPrompt)>,
    settings: Settings,
    bg_store: Arc<BackgroundTaskStore>,
    secrets: Option<Arc<dyn Vault>>,
}

impl Kernel {
    pub fn new(settings: Settings) -> Result<Self> {
        let bg_config = settings.bg_tasks.clone();
        let bg_store = BackgroundTaskStore::with_config(bg_config.clone());
        let tool_registry = ToolRegistry::new();
        loopal_tools::builtin::register_all(&tool_registry, bg_store.clone());

        let mut provider_registry = ProviderRegistry::new();
        provider_registry::register_providers(&settings, &mut provider_registry);

        let hook_registry = HookRegistry::new(settings.hooks.clone());
        let factory = Arc::new(DefaultExecutorFactory::new(None));
        let hook_service = HookService::new(hook_registry, factory);
        let mcp_manager = Arc::new(RwLock::new(McpManager::new()));

        spawn_bg_gc_tick(bg_store.clone(), bg_config);

        info!("kernel initialized");

        Ok(Self {
            tool_registry,
            provider_registry,
            hook_service,
            mcp_manager,
            mcp_instructions: Vec::new(),
            mcp_resources: Vec::new(),
            mcp_prompts: Vec::new(),
            settings,
            bg_store,
            secrets: None,
        })
    }

    /// Inject the resolved secret store. Subsequent operations that need
    /// secret access (MCP spawn, runtime tool args, prompt translation) use it.
    pub fn set_secrets(&mut self, store: Arc<dyn Vault>) {
        self.secrets = Some(store);
    }

    /// The currently-configured secret store, if any.
    pub fn secrets(&self) -> Option<&Arc<dyn Vault>> {
        self.secrets.as_ref()
    }

    /// Register an additional tool (thread-safe, can be called after Arc wrapping).
    pub fn register_tool(&self, tool: Box<dyn loopal_tool_api::Tool>) {
        self.tool_registry.register(tool);
    }

    /// Register thread-goal tools — only valid for the root agent that
    /// owns a `goal_session`. Sub-agents must not call this.
    pub fn register_goal_tools(&self) {
        loopal_tools::builtin::register_goal_tools(&self.tool_registry);
    }

    /// Register an additional provider (useful for testing with mock providers).
    pub fn register_provider(&mut self, provider: Arc<dyn loopal_provider_api::Provider>) {
        self.provider_registry.register(provider);
    }

    /// Start all configured MCP servers and register their tools.
    pub async fn start_mcp(&mut self) -> Result<()> {
        if !self.settings.mcp_servers.is_empty() {
            let mut mgr = self.mcp_manager.write().await;
            if let Some(ref store) = self.secrets {
                mgr.set_secrets(store.clone());
            }
            mgr.start_all(&self.settings.mcp_servers).await?;
            info!(
                count = self.settings.mcp_servers.len(),
                "MCP servers started"
            );

            let tools_with_server = mgr.get_tools_with_server();
            self.mcp_instructions = mgr.get_server_instructions();
            self.mcp_resources = mgr.get_resources();
            self.mcp_prompts = mgr.get_prompts();
            drop(mgr);

            let mut skipped_tools = Vec::new();
            for (server_name, tool_def) in tools_with_server {
                if self.tool_registry.get(&tool_def.name).is_some() {
                    warn!(
                        tool = %tool_def.name, server = %server_name,
                        "MCP tool name conflicts with existing tool, skipping"
                    );
                    skipped_tools.push(tool_def.name.clone());
                    continue;
                }
                info!(tool = %tool_def.name, server = %server_name, "registering MCP tool");
                let adapter =
                    McpToolAdapter::new(tool_def, server_name, Arc::clone(&self.mcp_manager));
                self.tool_registry.register(Box::new(adapter));
            }

            if !skipped_tools.is_empty() {
                let mut mgr = self.mcp_manager.write().await;
                for name in &skipped_tools {
                    mgr.remove_tool_mapping(name);
                }
            }
        }
        Ok(())
    }

    pub fn create_backend(
        &self,
        cwd: &std::path::Path,
        session_id: &str,
    ) -> Arc<dyn loopal_tool_api::Backend> {
        use loopal_config::SandboxPolicy;
        let policy = if self.settings.sandbox.policy != SandboxPolicy::Disabled {
            Some(loopal_sandbox::resolve_policy(&self.settings.sandbox, cwd))
        } else {
            None
        };
        let limits = loopal_backend::ResourceLimits {
            image_max_bytes: self.settings.images.max_bytes,
            image_max_pixels: self.settings.images.max_pixels,
            ..loopal_backend::ResourceLimits::default()
        };
        loopal_backend::LocalBackend::new(cwd.to_path_buf(), policy, limits, session_id)
    }

    pub fn bg_store(&self) -> &Arc<BackgroundTaskStore> {
        &self.bg_store
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn loopal_tool_api::Tool>> {
        self.tool_registry.get(name)
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_registry.to_definitions()
    }

    pub fn resolve_provider(
        &self,
        model: &str,
    ) -> std::result::Result<Arc<dyn loopal_provider_api::Provider>, loopal_error::LoopalError>
    {
        self.provider_registry.resolve(model)
    }

    pub fn get_hooks(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
    ) -> Vec<&loopal_config::HookConfig> {
        self.hook_service
            .registry()
            .match_hooks(event, tool_name, None)
    }

    pub fn hook_service(&self) -> &HookService {
        &self.hook_service
    }

    pub fn mcp_manager(&self) -> &Arc<RwLock<McpManager>> {
        &self.mcp_manager
    }
}
