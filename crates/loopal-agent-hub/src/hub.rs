//! Hub — thin coordination layer over AgentRegistry + UiDispatcher.
//!
//! Agents and UI clients are managed by separate subsystems.
//! Hub ties them together: agent events flow to UI via broadcast,
//! permission requests flow from agents to UI clients via relay.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Notify, mpsc};

use loopal_protocol::AgentEvent;

use crate::agent_registry::AgentRegistry;
use crate::pending_relay::{PendingPermissionInfo, PendingQuestionInfo};
use crate::ui_dispatcher::UiDispatcher;
use crate::uplink::HubUplink;

pub struct Hub {
    pub registry: AgentRegistry,
    pub ui: UiDispatcher,
    pub uplink: Option<Arc<HubUplink>>,
    pub listener_port: Option<u16>,
    /// Auth token printed by the bootstrapping process; required by
    /// `--attach-hub` clients. `None` for in-process tests / before the
    /// listener has started.
    pub listener_token: Option<String>,
    pub max_total_agents: u32,
    pub default_cwd: PathBuf,
    /// Agent permission requests suspended awaiting UI response. Keyed by
    /// `(agent_name, tool_call_id)` so cross-agent reuse of the same
    /// `tool_call_id` does not overwrite pending state.
    pub pending_permissions: HashMap<(String, String), PendingPermissionInfo>,
    /// Agent question requests suspended awaiting UI response. Keyed by
    /// `(agent_name, question_id)`.
    pub pending_questions: HashMap<(String, String), PendingQuestionInfo>,
    /// Fired when an external `hub/shutdown` request arrives. The
    /// standalone `--hub-only` driver awaits on this to know when to
    /// tear down agents and exit. In-process Hubs ignore it.
    pub shutdown_signal: Arc<Notify>,
}

impl Hub {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self::with_cwd(event_tx, PathBuf::from("."))
    }

    /// Construct a Hub with an explicit `default_cwd` for cross-hub spawns.
    /// Production callers should pass the directory the Hub process was
    /// started in. Path is canonicalized so child processes spawned with
    /// this cwd see an absolute path independent of their inherited cwd.
    pub fn with_cwd(event_tx: mpsc::Sender<AgentEvent>, default_cwd: PathBuf) -> Self {
        let canonical = match default_cwd.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    path = %default_cwd.display(),
                    error = %e,
                    "Hub::with_cwd: canonicalize failed, using path verbatim — \
                     cross-hub spawns may inherit unpredictable cwd"
                );
                default_cwd
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
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
            shutdown_signal: Arc::new(Notify::new()),
        }
    }

    /// Create a no-op Hub (for tests that don't need real connections).
    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self::with_cwd(tx, PathBuf::from("."))
    }
}
