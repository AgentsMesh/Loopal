use chrono::Utc;
use tokio::io::AsyncWriteExt as _;
use tracing::{error, info};

use crate::cli::Cli;

use super::{discovery, token_channel};

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<()> {
    info!("starting in hub-only mode");

    let ctx = match super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config, resume).await {
        Ok(ctx) => ctx,
        Err(e) => {
            write_handshake_error(&e.to_string()).await;
            return Err(e);
        }
    };

    let port = match ctx.hub.lock().await.listener_port {
        Some(p) => p,
        None => {
            let msg = "hub listener has no port";
            write_handshake_error(msg).await;
            return Err(anyhow::anyhow!(msg));
        }
    };
    let token = ctx.hub_token.clone();
    let addr = format!("127.0.0.1:{port}");
    let pid = std::process::id();

    // Bind the per-pid token channel FIRST. If it fails the discovery
    // record is intentionally NOT written so `--list-hubs` does not
    // advertise an unreachable Hub. Users can still attach via the
    // explicit `--attach-hub <addr> --hub-token <t>` path printed on
    // stdout.
    let channel_result = token_channel::bind_token_channel(pid, token.clone());
    let _channel = match channel_result {
        Ok(handle) => {
            let record = discovery::HubDiscoveryRecord {
                pid,
                tcp_addr: addr.clone(),
                cwd: cwd.display().to_string(),
                started_at: Utc::now().to_rfc3339(),
                root_session_id: ctx.root_session_id.clone(),
            };
            if let Err(e) = discovery::write_record(&record) {
                error!(error = %e, "failed to write hub discovery record");
            }
            Some(handle)
        }
        Err(e) => {
            error!(error = %e, "failed to bind hub token channel; \
                                  --list-hubs / --attach-hub-pid unavailable");
            None
        }
    };

    write_handshake_line(&addr, &token, &ctx.root_session_id).await?;
    info!(%addr, pid, "hub-only listening; awaiting hub/shutdown");

    // Subscribe to events BEFORE starting the broadcast forwarder, so
    // the persist watcher does not miss any SubAgentSpawned that could
    // race with the loop's first iteration.
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

async fn write_handshake_line(addr: &str, token: &str, session_id: &str) -> anyhow::Result<()> {
    // Use tokio's async stdout: writing the handshake on the runtime's
    // sync std::io::stdout would block the reactor if the parent's pipe
    // buffer were full (would deadlock the spawn handshake itself).
    let mut out = tokio::io::stdout();
    let line = format!("LOOPAL_HUB {addr} {token} {session_id}\n");
    out.write_all(line.as_bytes()).await.map_err(|e| {
        error!(error = %e, "failed to write hub handshake to stdout");
        anyhow::anyhow!("hub handshake write failed: {e}")
    })?;
    let _ = out.flush().await;
    Ok(())
}

async fn write_handshake_error(msg: &str) {
    let sanitized: String = msg
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    let mut out = tokio::io::stdout();
    let line = format!("LOOPAL_HUB_ERROR {sanitized}\n");
    let _ = out.write_all(line.as_bytes()).await;
    let _ = out.flush().await;
}
