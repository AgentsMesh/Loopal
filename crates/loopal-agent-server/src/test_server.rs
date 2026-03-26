//! Test-injectable server loop — accepts mock provider for integration tests.

use std::path::PathBuf;
use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_runtime::agent_loop;

use crate::params::{self, StartParams};
use crate::server::wait_for_initialize;

/// Run the server with injected mock provider (for integration tests).
#[doc(hidden)]
pub async fn run_server_for_test(
    transport: Arc<dyn Transport>,
    provider: Arc<dyn loopal_provider_api::Provider>,
    cwd: PathBuf,
    session_dir: PathBuf,
) -> anyhow::Result<()> {
    let connection = Arc::new(Connection::new(transport));
    let mut incoming_rx = connection.start();
    wait_for_initialize(&connection, &mut incoming_rx).await?;
    loop {
        let Some(msg) = incoming_rx.recv().await else {
            break;
        };
        match msg {
            Incoming::Request {
                id,
                method,
                params: p,
            } if method == methods::AGENT_START.name => {
                let start = StartParams {
                    cwd: p["cwd"].as_str().map(String::from),
                    model: p["model"].as_str().map(String::from),
                    mode: p["mode"].as_str().map(String::from),
                    prompt: p["prompt"].as_str().map(String::from),
                    permission_mode: p["permission_mode"].as_str().map(String::from),
                    no_sandbox: p["no_sandbox"].as_bool().unwrap_or(false),
                };
                let agent_params = params::build_with_provider(
                    &cwd,
                    &start,
                    &connection,
                    incoming_rx,
                    provider,
                    &session_dir,
                )?;
                let _ = connection
                    .respond(
                        id,
                        serde_json::json!({"session_id": agent_params.session.id}),
                    )
                    .await;
                let _ = agent_loop(agent_params).await;
                break;
            }
            Incoming::Request { id, .. } => {
                let _ = connection
                    .respond_error(id, jsonrpc::METHOD_NOT_FOUND, "expected agent/start")
                    .await;
            }
            Incoming::Notification { .. } => {}
        }
    }
    Ok(())
}
