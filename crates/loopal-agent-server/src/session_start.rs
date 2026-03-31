//! Session creation — handles `agent/start` by building HubFrontend + agent loop.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_config::load_config;
use loopal_ipc::connection::Connection;
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_loop;

use crate::agent_setup;
use crate::hub_frontend::HubFrontend;
use crate::params::StartParams;
use crate::session_hub::{InputFromClient, SessionHub, SharedSession};

/// Handle returned to the dispatch loop after starting a session.
pub(crate) struct SessionHandle {
    pub session_id: String,
    pub session: Arc<SharedSession>,
    pub agent_task: tokio::task::JoinHandle<()>,
    /// When false (prompt-driven session), the server process exits after agent completes.
    /// When true (no initial prompt), the server stays alive for subsequent messages.
    pub has_initial_prompt: bool,
}

/// Create a session: build Kernel, HubFrontend, spawn agent loop.
pub(crate) async fn start_session(
    connection: &Arc<Connection>,
    request_id: i64,
    params: serde_json::Value,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<SessionHandle> {
    let cwd_str = params["cwd"].as_str().map(String::from);
    let cwd = cwd_str
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let start = StartParams {
        cwd: cwd_str,
        model: params["model"].as_str().map(String::from),
        mode: params["mode"].as_str().map(String::from),
        prompt: params["prompt"].as_str().map(String::from),
        permission_mode: params["permission_mode"].as_str().map(String::from),
        no_sandbox: params["no_sandbox"].as_bool().unwrap_or(false),
        resume: params["resume"].as_str().map(String::from),
    };

    let mut config = load_config(&cwd)?;
    crate::params::apply_start_overrides(&mut config.settings, &start);
    let kernel = if is_production {
        crate::params::build_kernel_from_config(&config, true).await?
    } else {
        match hub.get_test_provider().await {
            Some(provider) => crate::params::build_kernel_with_provider(provider)?,
            None => crate::params::build_kernel_from_config(&config, false).await?,
        }
    };

    // Create session infrastructure
    let (input_tx, input_rx) = tokio::sync::mpsc::channel::<InputFromClient>(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
    let interrupt_tx = Arc::new(watch_tx);

    let frontend_placeholder = Arc::new(HubFrontend::new(
        Arc::new(SharedSession::placeholder(
            input_tx.clone(),
            interrupt.clone(),
            interrupt_tx.clone(),
        )),
        input_rx,
        None,
        watch_rx,
    ));

    // If a prompt was provided, the server process should exit after agent completes.
    let has_initial_prompt = start.prompt.is_some();

    let agent_params = agent_setup::build_with_frontend(
        &cwd,
        &config,
        &start,
        frontend_placeholder.clone(),
        interrupt.clone(),
        interrupt_tx.clone(),
        kernel,
        connection.clone(),
        None,
    )?;

    let session_id = agent_params.session.id.clone();

    let session = Arc::new(SharedSession {
        session_id: session_id.clone(),
        clients: Mutex::new(Vec::new()),
        input_tx,
        interrupt: interrupt.clone(),
        interrupt_tx: interrupt_tx.clone(),
    });
    session.add_client("stdio".into(), connection.clone()).await;
    frontend_placeholder.replace_session(session.clone()).await;
    hub.register_session(session.clone()).await;

    let _ = connection
        .respond(request_id, serde_json::json!({"session_id": session_id}))
        .await;
    info!(session = %session_id, "session started");

    let agent_task = tokio::spawn(async move {
        match agent_loop(agent_params).await {
            Ok(output) => info!(reason = ?output.terminate_reason, "agent loop completed"),
            Err(e) => tracing::error!(error = %e, "agent loop error"),
        }
    });

    Ok(SessionHandle {
        session_id,
        session,
        agent_task,
        has_initial_prompt,
    })
}
