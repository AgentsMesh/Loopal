//! `AgentSetupContext` — context bag passed to `build_with_frontend`.
//!
//! Split out so `agent_setup.rs` (the actual setup pipeline) stays
//! within the project's 200-LOC budget. The struct intentionally keeps
//! `pub` fields for ergonomic destructuring inside the crate, while
//! `#[non_exhaustive]` blocks external struct-literal init.

use std::sync::Arc;

use loopal_config::ResolvedConfig;
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_runtime::frontend::traits::AgentFrontend;

use crate::params::StartParams;

/// Aggregate inputs for [`crate::agent_setup::build_with_frontend`].
///
/// Every required dependency for the agent setup pipeline. Pre-existing
/// callers passed 10+ positional arguments; this struct collapses that
/// into a single value so the builder signature stays readable when new
/// dependencies arrive.
///
/// `#[non_exhaustive]` forces external callers (notably integration
/// tests) to use [`AgentSetupContext::new`] so adding a new field
/// doesn't silently miss test sites.
#[non_exhaustive]
pub struct AgentSetupContext<'a> {
    pub cwd: &'a std::path::Path,
    pub config: &'a ResolvedConfig,
    pub start: &'a StartParams,
    pub frontend: Arc<dyn AgentFrontend>,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    pub kernel: Arc<Kernel>,
    pub hub_connection: Arc<loopal_ipc::connection::Connection>,
    pub session_dir_override: Option<&'a std::path::Path>,
    pub hub: &'a crate::session_hub::SessionHub,
}

impl<'a> AgentSetupContext<'a> {
    /// Construct with every required dependency.
    ///
    /// Prefer this over `pub` field struct-literal init from external
    /// crates: any new field added here forces every caller to update,
    /// rather than picking up a default that may not be what they want.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: &'a std::path::Path,
        config: &'a ResolvedConfig,
        start: &'a StartParams,
        frontend: Arc<dyn AgentFrontend>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
        kernel: Arc<Kernel>,
        hub_connection: Arc<loopal_ipc::connection::Connection>,
        session_dir_override: Option<&'a std::path::Path>,
        hub: &'a crate::session_hub::SessionHub,
    ) -> Self {
        Self {
            cwd,
            config,
            start,
            frontend,
            interrupt,
            interrupt_tx,
            kernel,
            hub_connection,
            session_dir_override,
            hub,
        }
    }
}
