use tokio::sync::oneshot;
use tracing::info;

use crate::cli::Cli;

use super::hub_registration::HubRegistration;
use super::startup_protocol::StartupProtocol;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<()> {
    run_with_protocol(cli, cwd, config, resume, StartupProtocol::HubOnly).await
}

pub async fn run_desktop(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
    parent_pid: Option<u32>,
) -> anyhow::Result<()> {
    run_with_protocol(
        cli,
        cwd,
        config,
        resume,
        StartupProtocol::Desktop { parent_pid },
    )
    .await
}

async fn run_with_protocol(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
    startup_protocol: StartupProtocol,
) -> anyhow::Result<()> {
    info!(?startup_protocol, "starting Hub-backed host mode");

    let (alive_tx, alive_rx) = oneshot::channel();
    let bootstrap = async {
        let prepared = super::hub_bootstrap::prepare_hub_and_agent_with_alive(
            cli,
            cwd,
            config,
            Some(alive_tx),
        )
        .await?;
        let requires_ui = cli.parent_only.require_ui_ready
            || matches!(startup_protocol, StartupProtocol::Desktop { .. });
        if requires_ui {
            super::ui_ready_gate::wait_for_interactive_ui(&prepared).await?;
        }
        super::hub_bootstrap::start_prepared_hub_and_agent(prepared, cli, cwd, config, resume).await
    };
    let alive_task = tokio::spawn(async move {
        if let Ok(info) = alive_rx.await {
            startup_protocol.write_alive(info.addr, info.token).await;
        }
    });

    let bootstrap_result = match startup_protocol {
        StartupProtocol::Desktop {
            parent_pid: Some(parent_pid),
        } => {
            tokio::select! {
                result = bootstrap => result,
                () = super::parent_liveness::wait_until_exit(parent_pid) => {
                    Err(anyhow::anyhow!(
                        "desktop parent process {parent_pid} exited during startup"
                    ))
                }
            }
        }
        _ => bootstrap.await,
    };
    let ctx = match bootstrap_result {
        Ok(ctx) => {
            let _ = alive_task.await;
            ctx
        }
        Err(e) => {
            let _ = alive_task.await;
            startup_protocol
                .write_error("startup_failed", e.to_string())
                .await;
            return Err(e);
        }
    };
    if resume.is_none() {
        startup_protocol
            .write_session_created(&ctx.root_session_id)
            .await;
    }

    if let Err(error) = super::root_view_ready::wait(&ctx.hub).await {
        startup_protocol
            .write_error("root_view_not_ready", error.to_string())
            .await;
        return Err(error);
    }

    let port = match ctx.hub.lock().await.listener_port {
        Some(p) => p,
        None => {
            let msg = "hub listener has no port";
            startup_protocol
                .write_error("listener_unavailable", msg)
                .await;
            return Err(anyhow::anyhow!(msg));
        }
    };
    let token = ctx.hub_token.clone();
    let addr = format!("127.0.0.1:{port}");
    let pid = std::process::id();

    let mut registration = HubRegistration::register(pid, &token, &addr, cwd, &ctx.root_session_id);

    startup_protocol
        .write_ready(&addr, &token, &ctx.root_session_id)
        .await;
    if matches!(startup_protocol, StartupProtocol::Desktop { .. }) {
        // Let the Desktop register and take its initial snapshot before the
        // synchronous session-directory scan begins.
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            super::cleanup_bash_log_orphans().await;
        });
    }
    info!(%addr, pid, "hub-only listening; awaiting hub/shutdown");

    let persist_subscriber = ctx.hub.lock().await.ui.subscribe_events();
    let persist_root = ctx.root_session_id.clone();
    tokio::spawn(async move {
        super::sub_agent_resume::hub_persist_watcher(persist_subscriber, persist_root).await;
    });

    let shutdown = ctx.hub.lock().await.shutdown_signal.clone();
    let parent_exited = match startup_protocol {
        StartupProtocol::Desktop {
            parent_pid: Some(parent_pid),
        } => {
            tokio::select! {
                () = shutdown.notified() => false,
                () = super::parent_liveness::wait_until_exit(parent_pid) => true,
            }
        }
        _ => {
            shutdown.notified().await;
            false
        }
    };

    if parent_exited {
        info!("desktop parent exited, draining agent");
    } else {
        info!("hub/shutdown received, draining agent");
    }
    registration.withdraw();
    let _ = ctx.agent_proc.shutdown().await;
    Ok(())
}
