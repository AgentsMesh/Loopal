//! Agent server for multi-process architecture.
//!
//! Activated via `loopal --serve`. Runs the agent loop in a dedicated process,
//! communicating with the TUI (or any IPC client) via JSON-RPC over stdio.
//!
//! This is the "Renderer Process" in the Chromium analogy — it owns the Kernel,
//! LLM providers, tools, and context pipeline.

#[doc(hidden)]
pub mod interrupt_filter;
mod ipc_frontend;
mod memory_adapter;
mod mock_loader;
mod params;
mod server;
mod test_server;

pub use server::{run_agent_server, run_agent_server_with_mock};
pub use test_server::run_server_for_test;

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
