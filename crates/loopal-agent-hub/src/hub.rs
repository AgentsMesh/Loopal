use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Notify, mpsc};

use loopal_hub_vault::HubVaultService;
use loopal_protocol::{AgentEvent, DEFAULT_INTERACTION_LIFETIME};

use crate::agent_registry::AgentRegistry;
use crate::mcp_service::HubMcpService;
use crate::pending_relay::{
    PendingPermissionInfo, PendingPlanApprovalInfo, PendingQuestionInfo, PendingRemoteQuestionInfo,
};
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
    pub pending_plan_approvals: HashMap<(String, String), PendingPlanApprovalInfo>,
    /// Destination-side authoritative relay records, keyed by
    /// `(qualified_agent, interaction_id)`.
    pub pending_remote_questions: HashMap<(String, String), PendingRemoteQuestionInfo>,
    /// Reducers for qualified remote agents. These make `view/snapshot`
    /// recover remote questions after UI lag/reconnect.
    pub remote_views: HashMap<String, Arc<Mutex<loopal_view_state::ViewStateReducer>>>,
    pending_interaction_timeout: Duration,
    pub session_permission_grants: HashSet<(String, String)>,
    pub shutdown_signal: Arc<Notify>,
    pub workspace: Option<Arc<loopal_workspace::WorkspaceService>>,
    pub user_config_dir: Option<PathBuf>,
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
        let workspace = match loopal_workspace::WorkspaceService::new(&canonical) {
            Ok(service) => Some(service),
            Err(error) => {
                tracing::error!(%error, "workspace service unavailable");
                None
            }
        };
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
            pending_plan_approvals: HashMap::new(),
            pending_remote_questions: HashMap::new(),
            remote_views: HashMap::new(),
            pending_interaction_timeout: DEFAULT_INTERACTION_LIFETIME,
            session_permission_grants: HashSet::new(),
            shutdown_signal: Arc::new(Notify::new()),
            workspace,
            user_config_dir: loopal_config::global_config_dir().ok(),
        }
    }

    pub fn set_vault_service(&mut self, vault: Arc<HubVaultService>) {
        self.vault_service = Some(vault);
    }

    pub fn set_mcp_service(&mut self, mcp: Arc<HubMcpService>) {
        self.mcp_service = mcp;
    }

    pub fn set_pending_interaction_timeout(&mut self, timeout: Duration) {
        self.pending_interaction_timeout = timeout;
    }

    pub(crate) fn pending_interaction_timeout(&self) -> Duration {
        self.pending_interaction_timeout
    }

    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self::with_cwd(tx, PathBuf::from("."))
    }
}
