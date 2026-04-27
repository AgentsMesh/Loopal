//! Session creation — handles `agent/start` by building HubFrontend + agent loop.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{Instrument, info};

use loopal_config::load_config;
use loopal_error::AgentOutput;
use loopal_ipc::connection::Connection;
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::agent_loop;

use crate::agent_setup;
use crate::hub_frontend::HubFrontend;
use crate::params::StartParams;
use crate::session_hub::{SessionHub, SharedSession};

/// Handle returned to the dispatch loop after starting a session.
pub(crate) struct SessionHandle {
    pub session_id: String,
    pub session: Arc<SharedSession>,
    pub agent_task: tokio::task::JoinHandle<Option<AgentOutput>>,
    /// Lifecycle mode — Ephemeral exits after completion, Persistent stays alive.
    pub lifecycle: loopal_runtime::LifecycleMode,
}

/// Create a session: build Kernel, HubFrontend, spawn agent loop.
pub(crate) async fn start_session(
    connection: &Arc<Connection>,
    request_id: i64,
    params: serde_json::Value,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<SessionHandle> {
    let session_span = tracing::info_span!("session_start", session.id = tracing::field::Empty);
    async {
        let cwd_str = params["cwd"].as_str().map(String::from);
        let cwd = cwd_str
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Lifecycle: explicit from params, default based on prompt presence.
        let lifecycle = match params["lifecycle"].as_str() {
            Some("ephemeral") => loopal_runtime::LifecycleMode::Ephemeral,
            Some("persistent") => loopal_runtime::LifecycleMode::Persistent,
            Some(unknown) => {
                anyhow::bail!(
                    "unknown lifecycle mode: '{unknown}' (expected 'ephemeral' or 'persistent')"
                );
            }
            None if params["prompt"].as_str().is_some() => loopal_runtime::LifecycleMode::Ephemeral,
            None => loopal_runtime::LifecycleMode::Persistent,
        };

        let start = StartParams {
            cwd: cwd_str,
            model: params["model"].as_str().map(String::from),
            mode: params["mode"].as_str().map(String::from),
            prompt: params["prompt"].as_str().map(String::from),
            permission_mode: params["permission_mode"].as_str().map(String::from),
            no_sandbox: params["no_sandbox"].as_bool().unwrap_or(false),
            resume: params["resume"].as_str().map(String::from),
            lifecycle,
            agent_type: params["agent_type"].as_str().map(String::from),
            depth: params["depth"].as_u64().map(|v| v as u32),
            fork_context: params.get("fork_context").cloned(),
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
        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<AgentInput>(16);
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

        let session_dir_override = hub.session_dir_override().await;
        let kernel_for_bridge = kernel.clone();
        let setup =
            agent_setup::build_with_frontend(crate::agent_setup_context::AgentSetupContext::new(
                &cwd,
                &config,
                &start,
                frontend_placeholder.clone(),
                interrupt.clone(),
                interrupt_tx.clone(),
                kernel,
                connection.clone(),
                session_dir_override.as_deref(),
                hub,
            ))
            .await?;
        let agent_params = setup.params;
        let task_store_for_bridge = setup.task_store;
        let scheduler_for_bridge = setup.scheduler;

        // Bind the scheduler to this session's id. Idempotent and
        // unifies fresh-session and resumed-session code paths through
        // a single SessionScopedCronStorage lookup.
        if let Err(e) = scheduler_for_bridge
            .switch_session(&agent_params.session().id)
            .await
        {
            tracing::warn!(error = %e, "failed to bind scheduler to session");
        }

        let session_id = agent_params.session().id.clone();
        tracing::Span::current().record("session.id", session_id.as_str());

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
        info!(session.id = %session_id, "session started");

        let spawn_rx = kernel_for_bridge.bg_store().subscribe_spawns();
        let bridge_task = crate::bg_task_bridge::spawn(spawn_rx, frontend_placeholder.clone());
        let task_change_rx = task_store_for_bridge.subscribe();
        let task_bridge_task = crate::task_bridge::spawn(
            task_change_rx,
            task_store_for_bridge,
            frontend_placeholder.clone(),
        );
        let cron_bridge_task =
            crate::cron_bridge::spawn(scheduler_for_bridge, frontend_placeholder.clone());

        let agent_task = tokio::spawn(async move {
            match agent_loop(agent_params).await {
                Ok(output) => {
                    info!(reason = ?output.terminate_reason, "agent loop completed");
                    Some(output)
                }
                Err(e) => {
                    tracing::error!(error = %e, "agent loop error");
                    None
                }
            }
        });

        let agent_task = {
            let bridge_abort = bridge_task.abort_handle();
            let task_bridge_abort = task_bridge_task.abort_handle();
            let cron_bridge_abort = cron_bridge_task.abort_handle();
            tokio::spawn(async move {
                let result = agent_task.await;
                bridge_abort.abort();
                task_bridge_abort.abort();
                cron_bridge_abort.abort();
                result.ok().flatten()
            })
        };

        Ok(SessionHandle {
            session_id,
            session,
            agent_task,
            lifecycle,
        })
    }
    .instrument(session_span)
    .await
}
