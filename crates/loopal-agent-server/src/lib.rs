//! Agent server for multi-process architecture.
//!
//! Activated internally via hidden `--serve` flag (set by parent process).
//! Runs the agent loop in a dedicated process,
//! communicating with consumers via JSON-RPC over stdio.
//!
//! This is the "Renderer Process" in the Chromium analogy — it owns the Kernel,
//! LLM providers, tools, and context pipeline.

mod agent_setup;
mod hub_emitter;
#[doc(hidden)]
pub mod hub_frontend;
#[doc(hidden)]
pub mod interrupt_filter;
mod ipc_emitter;
mod ipc_frontend;
mod memory_adapter;
mod mock_loader;
mod params;
mod server;
pub mod server_info;
mod session_forward;
#[doc(hidden)]
pub mod session_hub;
mod session_start;
mod test_server;

pub use server::{run_agent_server, run_agent_server_with_mock};
pub use test_server::{run_server_for_test, run_server_for_test_interactive, run_test_connection};

/// Test-only constructor for IpcFrontend (used by integration tests).
#[doc(hidden)]
pub fn ipc_frontend_for_test(
    connection: std::sync::Arc<loopal_ipc::connection::Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<loopal_ipc::connection::Incoming>,
) -> std::sync::Arc<dyn loopal_runtime::frontend::traits::AgentFrontend> {
    std::sync::Arc::new(ipc_frontend::IpcFrontend::new(
        connection,
        incoming_rx,
        None,
    ))
}

/// Test-only: create a HubFrontend with a SharedSession for integration tests.
#[doc(hidden)]
pub fn hub_frontend_for_test(
    session: std::sync::Arc<session_hub::SharedSession>,
    input_rx: tokio::sync::mpsc::Receiver<session_hub::InputFromClient>,
    interrupt_rx: tokio::sync::watch::Receiver<u64>,
) -> std::sync::Arc<dyn loopal_runtime::frontend::traits::AgentFrontend> {
    std::sync::Arc::new(hub_frontend::HubFrontend::new(
        session,
        input_rx,
        None,
        interrupt_rx,
    ))
}

/// Test-only: re-export internal types for integration tests.
#[doc(hidden)]
pub mod testing {
    pub use crate::agent_setup::build_with_frontend;
    pub use crate::params::{StartParams, build_kernel_with_provider};
    pub use crate::session_hub::{InputFromClient, SharedSession};
}
