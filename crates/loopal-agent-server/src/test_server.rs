//! Test-injectable server loop — accepts mock provider for integration tests.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::transport::Transport;

use crate::server::{dispatch_loop, wait_for_initialize};
use crate::session_hub::SessionHub;

/// Run the server with injected mock provider (for integration tests).
#[doc(hidden)]
pub async fn run_server_for_test(
    transport: Arc<dyn Transport>,
    provider: Arc<dyn loopal_provider_api::Provider>,
    _cwd: std::path::PathBuf,
    _session_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    let connection = Arc::new(Connection::new(transport));
    let mut incoming_rx = connection.start();
    let hub = SessionHub::new();
    hub.set_test_provider(provider).await;
    wait_for_initialize(&connection, &mut incoming_rx).await?;
    dispatch_loop(connection, incoming_rx, &hub, false).await
}

/// Alias for ACP bridge tests.
#[doc(hidden)]
pub async fn run_server_for_test_interactive(
    transport: Arc<dyn Transport>,
    provider: Arc<dyn loopal_provider_api::Provider>,
    _cwd: std::path::PathBuf,
    _session_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    run_server_for_test(transport, provider, _cwd, _session_dir).await
}

/// Run a dispatch loop for a single connection on a shared hub.
/// Used in multi-connection tests (e.g. observer joins active session).
#[doc(hidden)]
pub async fn run_test_connection(
    transport: Arc<dyn Transport>,
    hub: Arc<SessionHub>,
) -> anyhow::Result<()> {
    let connection = Arc::new(Connection::new(transport));
    let mut incoming_rx = connection.start();
    wait_for_initialize(&connection, &mut incoming_rx).await?;
    dispatch_loop(connection, incoming_rx, &hub, false).await
}
