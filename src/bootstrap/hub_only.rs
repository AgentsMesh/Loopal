use chrono::Utc;
use tokio::io::AsyncWriteExt as _;
use tokio::sync::oneshot;
use tracing::{error, info};

use loopal_ipc::HandshakeLine;

use crate::cli::Cli;

use super::{discovery, token_channel};

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<()> {
    info!("starting in hub-only mode");

    let (alive_tx, alive_rx) = oneshot::channel();
    let bootstrap = super::hub_bootstrap::bootstrap_hub_and_agent_with_alive(
        cli,
        cwd,
        config,
        resume,
        Some(alive_tx),
    );
    let alive_task = tokio::spawn(async move {
        if let Ok(info) = alive_rx.await {
            write_line(&HandshakeLine::Alive {
                addr: info.addr,
                token: info.token,
            })
            .await;
        }
    });
    let ctx = match bootstrap.await {
        Ok(ctx) => {
            let _ = alive_task.await;
            ctx
        }
        Err(e) => {
            let _ = alive_task.await;
            write_line(&HandshakeLine::Error(e.to_string())).await;
            return Err(e);
        }
    };

    let port = match ctx.hub.lock().await.listener_port {
        Some(p) => p,
        None => {
            let msg = "hub listener has no port";
            write_line(&HandshakeLine::Error(msg.into())).await;
            return Err(anyhow::anyhow!(msg));
        }
    };
    let token = ctx.hub_token.clone();
    let addr = format!("127.0.0.1:{port}");
    let pid = std::process::id();

    let _channel = register_with_token_channel(pid, &token, &addr, cwd, &ctx.root_session_id);

    write_line(&HandshakeLine::Ready {
        session_id: ctx.root_session_id.clone(),
    })
    .await;
    write_line(&HandshakeLine::Legacy {
        addr: addr.clone(),
        token: token.clone(),
        session_id: ctx.root_session_id.clone(),
    })
    .await;
    info!(%addr, pid, "hub-only listening; awaiting hub/shutdown");

    let persist_subscriber = ctx.hub.lock().await.ui.subscribe_events();
    let persist_root = ctx.root_session_id.clone();
    tokio::spawn(async move {
        super::sub_agent_resume::hub_persist_watcher(persist_subscriber, persist_root).await;
    });

    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);

    let shutdown = ctx.hub.lock().await.shutdown_signal.clone();
    shutdown.notified().await;

    info!("hub/shutdown received, draining agent");
    let _ = ctx.agent_proc.shutdown().await;
    discovery::remove_record(pid);
    token_channel::cleanup_channel(pid);
    Ok(())
}

fn register_with_token_channel(
    pid: u32,
    token: &str,
    addr: &str,
    cwd: &std::path::Path,
    session_id: &str,
) -> Option<tokio::task::JoinHandle<()>> {
    // reason: bind the per-pid token channel FIRST. If it fails the discovery
    // record is intentionally NOT written so `--list-hubs` does not advertise
    // an unreachable Hub.
    let handle = match token_channel::bind_token_channel(pid, token.to_string()) {
        Ok(h) => h,
        Err(e) => {
            error!(error = %e, "failed to bind hub token channel; --list-hubs / --attach-hub-pid unavailable");
            return None;
        }
    };
    let record = discovery::HubDiscoveryRecord {
        pid,
        tcp_addr: addr.to_string(),
        cwd: cwd.display().to_string(),
        started_at: Utc::now().to_rfc3339(),
        root_session_id: session_id.to_string(),
    };
    if let Err(e) = discovery::write_record(&record) {
        error!(error = %e, "failed to write hub discovery record");
    }
    Some(handle)
}

async fn write_line(line: &HandshakeLine) {
    let mut out = tokio::io::stdout();
    if let Err(e) = out.write_all(line.encode().as_bytes()).await {
        error!(error = %e, "failed to write handshake line");
    }
    let _ = out.flush().await;
}
