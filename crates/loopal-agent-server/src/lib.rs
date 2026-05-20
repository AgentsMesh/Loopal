//! Agent server for multi-process architecture.
//!
//! Activated internally via hidden `--serve` flag (set by parent process).
//! Runs the agent loop in a dedicated process,
//! communicating with consumers via JSON-RPC over stdio.
//!
//! This is the "Renderer Process" in the Chromium analogy — it owns the Kernel,
//! LLM providers, tools, and context pipeline.

mod agent_loop_params_factory;
mod agent_setup;
mod agent_setup_context;
mod agent_setup_helpers;
mod bg_task_bridge;
mod bg_task_bridge_monitor;
mod bg_task_bridge_sampler;
mod connection_mcp_client;
mod cron_bridge;
#[doc(hidden)]
pub mod dispatch;
mod hub_broadcaster;
#[doc(hidden)]
pub mod hub_frontend;
mod hub_input_receiver;
#[doc(hidden)]
pub mod interrupt_filter;
mod ipc_handlers;
mod memory_adapter;
mod memory_consolidation;
mod mcp_dispatch;
mod mock_loader;
#[doc(hidden)]
pub mod params;
#[doc(hidden)]
pub mod prompt_post;
mod server;
pub mod server_info;
mod server_init;
mod session_forward;
mod session_handlers_factory;
#[doc(hidden)]
pub mod session_hub;
mod session_hub_storage;
mod session_resources;
mod session_spawn;
mod session_start;
mod shared_session;
mod spawn_policy;
mod task_bridge;
mod test_server;

pub use server::{run_agent_server, run_agent_server_with_mock};
pub use test_server::{run_server_for_test, run_server_for_test_interactive, run_test_connection};

#[doc(hidden)]
pub fn hub_frontend_for_test(
    session: std::sync::Arc<session_hub::SharedSession>,
    input_rx: tokio::sync::mpsc::Receiver<loopal_runtime::agent_input::AgentInput>,
    interrupt_rx: tokio::sync::watch::Receiver<u64>,
) -> std::sync::Arc<dyn loopal_runtime::frontend::traits::AgentFrontend> {
    let session_ref: ipc_handlers::SessionRef =
        std::sync::Arc::new(tokio::sync::RwLock::new(session));
    let perm: Box<dyn loopal_runtime::frontend::permission_handler::PermissionHandler> =
        Box::new(ipc_handlers::IpcPermissionHandler::new(session_ref.clone()));
    let question: Box<dyn loopal_runtime::frontend::question_handler::QuestionHandler> =
        Box::new(ipc_handlers::IpcQuestionHandler::new(session_ref.clone()));
    std::sync::Arc::new(hub_frontend::HubFrontend::new(
        session_ref,
        input_rx,
        None,
        interrupt_rx,
        perm,
        question,
    ))
}

#[doc(hidden)]
pub mod testing {
    pub use crate::agent_setup::build_with_frontend;
    pub use crate::agent_setup_context::AgentSetupContext;
    pub use crate::agent_setup_helpers::{
        build_initial_messages, build_model_router, collect_feature_tags, spawn_sub_agent_forwarder,
    };
    pub use crate::bg_task_bridge::spawn as bg_task_bridge_spawn;
    pub use crate::cron_bridge::spawn as cron_bridge_spawn;
    pub use crate::cron_bridge::spawn_with_receiver as cron_bridge_spawn_with_receiver;
    pub use crate::ipc_handlers::SessionRef;
    pub use crate::params::AgentSetupResult;
    pub use crate::params::{StartParams, apply_start_overrides, build_kernel_with_provider};
    pub use crate::session_handlers_factory::build_session_handlers;
    pub use crate::session_hub::{SessionHub, SharedSession};
    pub use crate::session_hub_storage::SessionHubError;
    pub use crate::session_resources::resolve_sessions_root;
    pub use crate::spawn_policy::build_depth_tool_filter;
    pub use loopal_runtime::agent_input::AgentInput;
}
