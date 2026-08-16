use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::{Mutex, Notify, mpsc};

use loopal_hub_vault::HubVaultService;
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{AgentCompletion, AgentEvent, DEFAULT_INTERACTION_LIFETIME, Envelope};

use crate::agent_registry::AgentRegistry;
use crate::mcp_service::HubMcpService;
use crate::pending_relay::{
    PendingPermissionInfo, PendingPlanApprovalInfo, PendingQuestionInfo, PendingRemoteQuestionInfo,
};
use crate::permission_receipts::PermissionReceiptRegistry;
use crate::spawn_registry::SpawnRegistry;
use crate::types::SpawnAuthority;
use crate::ui_dispatcher::UiDispatcher;
use crate::uplink::HubUplink;
use crate::workflow::WorkflowCoordinatorHandle;

pub struct Hub {
    pub registry: AgentRegistry,
    pub ui: UiDispatcher,
    pub uplink: Option<Arc<HubUplink>>,
    pub listener_port: Option<u16>,
    pub listener_token: Option<String>,
    pub max_total_agents: u32,
    pub max_agent_depth: u32,
    pub default_cwd: PathBuf,
    root_spawn_authority: SpawnAuthority,
    pub spawn_registry: Arc<SpawnRegistry>,
    pub protected_audit: Option<Arc<dyn loopal_vault_api::AuditSink>>,
    pub vault_service: Option<Arc<HubVaultService>>,
    pub mcp_service: Arc<HubMcpService>,
    pub pending_permissions: HashMap<(String, String), PendingPermissionInfo>,
    pub pending_questions: HashMap<(String, String), PendingQuestionInfo>,
    pub pending_plan_approvals: HashMap<(String, String), PendingPlanApprovalInfo>,
    /// Destination-side authoritative relay records, keyed by
    /// `(qualified_agent, interaction_id)`.
    pub pending_remote_questions: HashMap<(String, String), PendingRemoteQuestionInfo>,
    /// Generation/lease-bound cache preventing an instant remote completion
    /// from overtaking its caller-side `SubAgentSpawned` event.
    shadow_spawn_admissions: HashMap<String, ShadowSpawnAdmission>,
    /// Names terminalized after an indeterminate remote spawn outcome. Without
    /// a protocol-level remote generation id, the same uplink lease must not
    /// reuse these names: a late completion could otherwise finish the new
    /// registration.
    shadow_spawn_quarantines: HashMap<String, Weak<HubUplink>>,
    /// Reducers for qualified remote agents. These make `view/snapshot`
    /// recover remote questions after UI lag/reconnect.
    pub remote_views: HashMap<String, Arc<Mutex<loopal_view_state::ViewStateReducer>>>,
    pending_interaction_timeout: Duration,
    pub(crate) session_permission_grants: HashSet<crate::permission_grants::PermissionGrantKey>,
    pub(crate) permission_receipts: PermissionReceiptRegistry,
    pub shutdown_signal: Arc<Notify>,
    workflow_coordinator: Option<WorkflowCoordinatorHandle>,
    pub workspace: Option<Arc<loopal_workspace::WorkspaceService>>,
    pub user_config_dir: Option<PathBuf>,
    final_sink_redaction_seed: FinalSinkRedactionSeed,
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
        let final_sink_redaction_seed = FinalSinkRedactionSeed::new();
        let mcp_service = Arc::new(
            HubMcpService::new_with_redaction_seed(final_sink_redaction_seed.clone())
                .with_spawn_registry(spawn_registry.clone()),
        );
        let workspace = match loopal_workspace::WorkspaceService::new(&canonical) {
            Ok(service) => Some(service),
            Err(error) => {
                tracing::error!(%error, "workspace service unavailable");
                None
            }
        };
        Self {
            registry: AgentRegistry::new_with_redaction_seed(
                event_tx,
                final_sink_redaction_seed.clone(),
            ),
            ui: UiDispatcher::new(),
            uplink: None,
            listener_port: None,
            listener_token: None,
            max_total_agents: 16,
            max_agent_depth: 2,
            default_cwd: canonical,
            root_spawn_authority: SpawnAuthority::default(),
            spawn_registry,
            protected_audit: None,
            vault_service: None,
            mcp_service,
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
            pending_plan_approvals: HashMap::new(),
            pending_remote_questions: HashMap::new(),
            shadow_spawn_admissions: HashMap::new(),
            shadow_spawn_quarantines: HashMap::new(),
            remote_views: HashMap::new(),
            pending_interaction_timeout: DEFAULT_INTERACTION_LIFETIME,
            session_permission_grants: HashSet::new(),
            permission_receipts: PermissionReceiptRegistry::default(),
            shutdown_signal: Arc::new(Notify::new()),
            workflow_coordinator: None,
            workspace,
            user_config_dir: loopal_config::global_config_dir().ok(),
            final_sink_redaction_seed,
        }
    }

    pub fn set_protected_audit(&mut self, audit: Arc<dyn loopal_vault_api::AuditSink>) {
        self.protected_audit = Some(audit);
    }

    pub fn set_vault_service(&mut self, vault: Arc<HubVaultService>) {
        self.vault_service = Some(vault);
    }

    pub fn set_mcp_service(&mut self, mcp: Arc<HubMcpService>) {
        self.mcp_service = mcp;
    }

    pub fn final_sink_redaction_seed(&self) -> FinalSinkRedactionSeed {
        self.final_sink_redaction_seed.clone()
    }

    pub fn install_workflow_coordinator(&mut self, coordinator: WorkflowCoordinatorHandle) {
        self.workflow_coordinator = Some(coordinator);
    }

    pub fn clear_workflow_coordinator(&mut self) {
        self.workflow_coordinator = None;
    }

    pub(crate) fn workflow_coordinator(&self) -> Option<WorkflowCoordinatorHandle> {
        self.workflow_coordinator.clone()
    }

    pub fn set_root_spawn_authority(&mut self, settings: &loopal_config::Settings) {
        self.root_spawn_authority = SpawnAuthority {
            model: settings.model.clone(),
            permission_mode: settings.permission_mode,
            decision_mode: settings.decision_mode,
            sandbox_policy: settings.sandbox.policy,
        };
    }

    pub(crate) fn root_spawn_authority(&self) -> SpawnAuthority {
        self.root_spawn_authority.clone()
    }

    pub fn set_pending_interaction_timeout(&mut self, timeout: Duration) {
        self.pending_interaction_timeout = timeout;
    }

    pub(crate) fn pending_interaction_timeout(&self) -> Duration {
        self.pending_interaction_timeout
    }

    pub(crate) fn install_shadow_spawn_admission(
        &mut self,
        name: &str,
        generation: u64,
        uplink: Arc<HubUplink>,
    ) {
        self.shadow_spawn_admissions.insert(
            name.to_string(),
            ShadowSpawnAdmission {
                generation,
                uplink,
                completion: None,
            },
        );
    }

    pub(crate) fn cache_shadow_spawn_completion(
        &mut self,
        name: &str,
        completion: AgentCompletion,
        envelope: Envelope,
    ) -> bool {
        let (envelope, completion) = crate::completion_guard::canonicalize_agent_result(
            envelope,
            completion,
            &self.final_sink_redaction_seed,
        );
        let Some(admission) = self.shadow_spawn_admissions.get_mut(name) else {
            return false;
        };
        if self.registry.generation(name) != Some(admission.generation)
            || !self
                .uplink
                .as_ref()
                .is_some_and(|uplink| Arc::ptr_eq(uplink, &admission.uplink))
        {
            return false;
        }
        if admission.completion.is_none() {
            admission.completion = Some(CachedShadowCompletion {
                completion,
                envelope,
            });
        }
        true
    }

    pub(crate) fn take_shadow_spawn_completion(
        &mut self,
        name: &str,
        generation: u64,
        uplink: &Arc<HubUplink>,
    ) -> Option<CachedShadowCompletion> {
        let matches_admission = self
            .shadow_spawn_admissions
            .get(name)
            .is_some_and(|admission| {
                admission.generation == generation && Arc::ptr_eq(&admission.uplink, uplink)
            });
        if !matches_admission {
            return None;
        }
        let admission = self
            .shadow_spawn_admissions
            .remove(name)
            .expect("checked shadow admission disappeared");
        if self.registry.generation(name) != Some(generation) {
            tracing::warn!(agent = %name, generation, "discarding stale shadow spawn admission");
            return None;
        }
        admission.completion
    }

    pub(crate) fn shadow_name_is_quarantined(
        &mut self,
        name: &str,
        uplink: &Arc<HubUplink>,
    ) -> bool {
        let same_lease = self
            .shadow_spawn_quarantines
            .get(name)
            .and_then(Weak::upgrade)
            .is_some_and(|lease| Arc::ptr_eq(&lease, uplink));
        if !same_lease {
            self.shadow_spawn_quarantines.remove(name);
        }
        same_lease
    }

    pub(crate) fn quarantine_shadow_name(&mut self, name: &str, uplink: Arc<HubUplink>) {
        self.shadow_spawn_quarantines
            .insert(name.to_string(), Arc::downgrade(&uplink));
    }

    pub(crate) fn should_drop_quarantined_completion(
        &self,
        name: &str,
        connection: &Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    ) -> bool {
        self.shadow_spawn_quarantines
            .get(name)
            .and_then(Weak::upgrade)
            .is_some_and(|lease| {
                Arc::ptr_eq(lease.connection(), connection)
                    && self
                        .uplink
                        .as_ref()
                        .is_some_and(|active| Arc::ptr_eq(active, &lease))
            })
    }

    pub(crate) fn is_active_uplink_connection(
        &self,
        connection: &Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    ) -> bool {
        self.uplink
            .as_ref()
            .is_some_and(|uplink| Arc::ptr_eq(uplink.connection(), connection))
    }

    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self::with_cwd(tx, PathBuf::from("."))
    }
}

struct ShadowSpawnAdmission {
    generation: u64,
    uplink: Arc<HubUplink>,
    completion: Option<CachedShadowCompletion>,
}

pub(crate) struct CachedShadowCompletion {
    pub(crate) completion: AgentCompletion,
    pub(crate) envelope: Envelope,
}
