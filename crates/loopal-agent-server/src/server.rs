//! Agent server entry point — stdio-only IPC lifecycle + agent loop.
//! Activated internally via hidden `--serve` flag. Communicates with Hub via stdin/stdout.
//! Agent is a pure worker: no TCP listener, no server_info, no external ports.

use std::sync::Arc;

use tracing::info;

use crate::dispatch::{RpcErrorPayload, dispatch_simple, respond_with};
use crate::server_init::wait_for_initialize_with_token;
use crate::session_hub::SessionHub;
use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;

pub async fn run_agent_server() -> anyhow::Result<()> {
    info!("agent server starting (stdio mode)");
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    run_agent_server_on_transport(transport).await
}

async fn run_agent_server_on_transport(transport: Arc<dyn Transport>) -> anyhow::Result<()> {
    let (connection, incoming_rx) = Connection::new(transport).into_listening();
    let hub = Arc::new(SessionHub::new());
    run_connection(connection, incoming_rx, &hub).await
}

pub async fn run_agent_server_with_mock(mock_path: &str) -> anyhow::Result<()> {
    info!(mock_path, "agent server starting with mock provider");
    let provider = crate::mock_loader::load_mock_provider(mock_path)?;
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    run_agent_server_with_mock_provider(provider, transport).await
}

async fn run_agent_server_with_mock_provider(
    provider: Arc<dyn loopal_provider_api::Provider>,
    transport: Arc<dyn Transport>,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = std::env::var_os("LOOPAL_TEST_SESSION_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("loopal-test-sessions"));
    crate::test_server::run_server_for_test(transport, provider, cwd, session_dir).await
}

async fn run_connection(
    connection: Arc<Connection<Listening>>,
    mut incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    hub: &SessionHub,
) -> anyhow::Result<()> {
    wait_for_initialize_with_token(&connection, &mut incoming_rx, None).await?;
    dispatch_loop(connection, incoming_rx, hub, true).await
}

pub(crate) async fn dispatch_loop(
    connection: Arc<Connection<Listening>>,
    mut incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<()> {
    let exit = loop {
        let Some(msg) = incoming_rx.recv().await else {
            info!("connection closed");
            break None;
        };
        let Incoming::Request { id, method, params } = msg else {
            continue;
        };

        if method == methods::AGENT_START.name {
            let outcome = run_session(
                &connection,
                &mut incoming_rx,
                hub,
                is_production,
                id,
                params,
            )
            .await?;
            match outcome {
                SessionOutcome::ReadyForStart => continue,
                SessionOutcome::Exit(output, redaction_seed, result_limit) => {
                    break Some((output, redaction_seed, result_limit));
                }
            }
        }

        let should_break = method == methods::AGENT_SHUTDOWN.name;
        respond_with(&connection, id, dispatch_simple(&method, params, hub).await).await;
        if should_break {
            break None;
        }
    };
    let (output, redaction_seed, result_limit) = exit.unwrap_or_else(|| {
        (
            None,
            loopal_output_guard::FinalSinkRedactionSeed::new(),
            loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
        )
    });
    crate::agent_completion_wire::send(&connection, output.as_ref(), &redaction_seed, result_limit)
        .await;
    info!("server shutting down");
    Ok(())
}

async fn run_session(
    connection: &Arc<Connection<Listening>>,
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    hub: &SessionHub,
    is_production: bool,
    id: i64,
    params: serde_json::Value,
) -> anyhow::Result<SessionOutcome> {
    let mut handle =
        start_session_or_respond_error(connection, id, params, hub, is_production).await?;
    let mut forward_result =
        crate::session_forward::forward_loop(incoming_rx, connection, &mut handle).await;
    hub.remove_session(&handle.session_id).await;

    while let crate::session_forward::ForwardResult::NewStart {
        id: new_id,
        params: new_params,
    } = forward_result
    {
        info!("chained agent/start after session end");
        handle = start_session_or_respond_error(connection, new_id, new_params, hub, is_production)
            .await?;
        forward_result =
            crate::session_forward::forward_loop(incoming_rx, connection, &mut handle).await;
        hub.remove_session(&handle.session_id).await;
    }

    let agent_output = match forward_result {
        crate::session_forward::ForwardResult::Done(output) => output,
        crate::session_forward::ForwardResult::Shutdown => {
            info!("active session received shutdown, server exiting");
            return Ok(SessionOutcome::Exit(
                None,
                handle.redaction_seed,
                handle.completion_result_limit,
            ));
        }
        crate::session_forward::ForwardResult::NewStart { .. } => {
            unreachable!("agent/start chaining is exhausted above")
        }
    };

    if handle.lifecycle.is_one_shot() {
        info!("ephemeral session complete, server exiting");
        Ok(SessionOutcome::Exit(
            agent_output,
            handle.redaction_seed,
            handle.completion_result_limit,
        ))
    } else {
        info!("persistent session ended, ready for next");
        Ok(SessionOutcome::ReadyForStart)
    }
}

enum SessionOutcome {
    ReadyForStart,
    Exit(
        Option<loopal_error::AgentOutput>,
        loopal_output_guard::FinalSinkRedactionSeed,
        usize,
    ),
}

// reason: start_session responds OK to `id` on success but `?`-early-exits
// without responding. Without this wrapper, client.send_request hangs.
async fn start_session_or_respond_error(
    connection: &Arc<Connection<Listening>>,
    id: i64,
    params: serde_json::Value,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<crate::session_start::SessionHandle> {
    match crate::session_start::start_session(connection, id, params, hub, is_production).await {
        Ok(handle) => Ok(handle),
        Err(e) => {
            respond_with(
                connection,
                id,
                Err(RpcErrorPayload::internal(format!(
                    "session start failed: {e}"
                ))),
            )
            .await;
            Err(e)
        }
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
