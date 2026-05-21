use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Notify, mpsc};

use loopal_hub_vault::HubVaultService;
use loopal_protocol::AgentEvent;

use crate::agent_registry::AgentRegistry;
use crate::mcp_service::HubMcpService;
use crate::pending_relay::{PendingPermissionInfo, PendingQuestionInfo};
use crate::spawn_registry::SpawnRegistry;
use crate::ui_dispatcher::UiDispatcher;
use crate::uplink::HubUplink;

pub struct Hub {
    pub registry: AgentRegistry,
    pub ui: UiDispatcher,
    pub uplink: Option<Arc<HubUplink>>,
    pub listener_port: Option<u16>,
    pub listener_token: Option<String>,
    pub max_total_agents: u32,
    pub default_cwd: PathBuf,
    pub spawn_registry: Arc<SpawnRegistry>,
    pub vault_service: Option<Arc<HubVaultService>>,
    pub mcp_service: Arc<HubMcpService>,
    pub pending_permissions: HashMap<(String, String), PendingPermissionInfo>,
    pub pending_questions: HashMap<(String, String), PendingQuestionInfo>,
    pub shutdown_signal: Arc<Notify>,
}

impl Hub {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self::with_cwd(event_tx, PathBuf::from("."))
    }

    pub fn with_cwd(event_tx: mpsc::Sender<AgentEvent>, default_cwd: PathBuf) -> Self {
        let canonical = match default_cwd.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    path = %default_cwd.display(),
                    error = %e,
                    "Hub::with_cwd: canonicalize failed, using path verbatim"
                );
                default_cwd
            }
        };
        let spawn_registry = Arc::new(SpawnRegistry::new());
        let mcp_service =
            Arc::new(HubMcpService::new().with_spawn_registry(spawn_registry.clone()));
        Self {
            registry: AgentRegistry::new(event_tx),
            ui: UiDispatcher::new(),
            uplink: None,
            listener_port: None,
            listener_token: None,
            max_total_agents: 16,
            default_cwd: canonical,
            spawn_registry,
            vault_service: None,
            mcp_service,
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
            shutdown_signal: Arc::new(Notify::new()),
        }
    }

    pub fn set_vault_service(&mut self, vault: Arc<HubVaultService>) {
        self.vault_service = Some(vault);
    }

    pub fn set_mcp_service(&mut self, mcp: Arc<HubMcpService>) {
        self.mcp_service = mcp;
    }

    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self::with_cwd(tx, PathBuf::from("."))
    }
}
