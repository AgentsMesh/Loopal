mod mcp;

use std::sync::Arc;

use loopal_config::HookEvent;
use loopal_config::Settings;
use loopal_error::Result;
use loopal_hooks::{HookRegistry, HookService};
use loopal_mcp::types::{McpPrompt, McpResource};
use loopal_mcp::{LocalMcpProvider, McpManager, McpProvider, SamplingCallback};
use loopal_provider::ProviderRegistry;
use loopal_tool_api::ToolDefinition;
use loopal_tools::ToolRegistry;
use loopal_vault_api::Vault;
use tokio::sync::RwLock;
use tracing::info;

use crate::hook_factory::DefaultExecutorFactory;
use crate::provider_registry;

use loopal_tool_background::BackgroundTaskStore;

use crate::bg_gc::spawn_bg_gc_tick;

/// Where MCP capability comes from for this kernel.
/// `Local` owns connections (root agent); `Proxy` forwards over IPC (sub-agent).
pub(super) enum McpBackend {
    Local(Arc<LocalMcpProvider>),
    Proxy(Arc<dyn McpProvider>),
}

impl McpBackend {
    pub(super) fn provider(&self) -> Arc<dyn McpProvider> {
        match self {
            Self::Local(local) => local.clone(),
            Self::Proxy(proxy) => proxy.clone(),
        }
    }

    pub(super) fn local(&self) -> Option<&Arc<LocalMcpProvider>> {
        match self {
            Self::Local(local) => Some(local),
            Self::Proxy(_) => None,
        }
    }
}

pub struct Kernel {
    pub(super) tool_registry: ToolRegistry,
    provider_registry: ProviderRegistry,
    hook_service: HookService,
    pub(super) mcp: McpBackend,
    pub(super) mcp_instructions: Vec<(String, String)>,
    pub(super) mcp_resources: Vec<(String, McpResource)>,
    pub(super) mcp_prompts: Vec<(String, McpPrompt)>,
    pub(super) settings: Settings,
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
        let local = Arc::new(LocalMcpProvider::new(Arc::new(RwLock::new(
            McpManager::new(),
        ))));

        spawn_bg_gc_tick(bg_store.clone(), bg_config);

        info!("kernel initialized");

        Ok(Self {
            tool_registry,
            provider_registry,
            hook_service,
            mcp: McpBackend::Local(local),
            mcp_instructions: Vec::new(),
            mcp_resources: Vec::new(),
            mcp_prompts: Vec::new(),
            settings,
            bg_store,
            secrets: None,
        })
    }

    pub fn set_secrets(&mut self, store: Arc<dyn Vault>) {
        self.secrets = Some(store);
    }

    pub fn secrets(&self) -> Option<&Arc<dyn Vault>> {
        self.secrets.as_ref()
    }

    pub fn register_tool(&self, tool: Box<dyn loopal_tool_api::Tool>) {
        self.tool_registry.register(tool);
    }

    pub fn register_goal_tools(&self) {
        loopal_tools::builtin::register_goal_tools(&self.tool_registry);
    }

    pub fn register_provider(&mut self, provider: Arc<dyn loopal_provider_api::Provider>) {
        self.provider_registry.register(provider);
    }

    /// Install a sampling callback so MCP servers can issue LLM calls
    /// (e.g. the in-process Claude provider). No-op for sub-agent kernels
    /// whose backend is a remote proxy.
    pub async fn set_mcp_sampling(&self, callback: Arc<dyn SamplingCallback>) {
        if let Some(local) = self.mcp.local() {
            local.manager().write().await.set_sampling(callback);
        }
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

    /// Local MCP manager (root agent only — None for sub-agents whose
    /// backend is a remote proxy).
    pub fn mcp_manager(&self) -> Option<Arc<RwLock<McpManager>>> {
        self.mcp.local().map(|l| l.manager())
    }

    /// Cloned reference to the local MCP provider. Used by late-registration
    /// listeners that must outlive `build_kernel_from_config`'s bounded wait.
    pub fn local_mcp_provider(&self) -> Option<Arc<LocalMcpProvider>> {
        self.mcp.local().cloned()
    }
}
