use std::time::Duration;

use tracing::{info, warn};

use crate::cli::Cli;

const HUB_EXIT_GRACE: Duration = Duration::from_secs(5);

/// Returns `Some(session_id)` for a normal exit (caller should print
/// `loopal --resume <id>` instructions), or `None` after `/detach-hub`
/// (Hub is still alive, re-attach instructions were printed instead).
pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<Option<String>> {
    info!("starting in unified Hub mode (spawn + attach)");
    let mut handshake = super::hub_spawn::spawn_hub_subprocess(cli, cwd, resume).await?;

    eprintln!("Hub listening on {}", handshake.addr);
    eprintln!("To attach a second TUI:");
    eprintln!(
        "  loopal --attach-hub {} --hub-token {}",
        handshake.addr, handshake.token
    );

    let outcome = match super::attach_mode::run_with_addr(
        cwd,
        config,
        &handshake.addr,
        &handshake.token,
        Some(&handshake.root_session_id),
        resume,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => {
            // Hub child is detached (setsid + kill_on_drop=false). On
            // attach failure we MUST kill it explicitly or it lives on
            // as an orphan that no one can reach.
            warn!(error = %e, "TUI attach failed; killing hub child to avoid orphan");
            let _ = handshake.child.kill().await;
            let _ = handshake.child.wait().await;
            return Err(e);
        }
    };
    if outcome.detached {
        // Hub keeps running; do not wait, do not reap. The child handle
        // drops here with `kill_on_drop(false)` so Hub continues.
        return Ok(None);
    }
    // Hub was asked to shut down (`/exit` or `/kill-hub`). Wait for the
    // child to actually exit before returning so callers (worktree
    // cleanup) do not race with agent flushes.
    match tokio::time::timeout(HUB_EXIT_GRACE, handshake.child.wait()).await {
        Ok(Ok(status)) => info!(?status, "hub child exited cleanly"),
        Ok(Err(e)) => warn!(error = %e, "hub child wait failed"),
        Err(_) => {
            // Tracing goes to a log file the user does not normally
            // read. Surface this on stderr so they know the Hub is
            // still alive and how to check.
            warn!(
                "hub child did not exit within {}s after shutdown; continuing",
                HUB_EXIT_GRACE.as_secs()
            );
            eprintln!();
            eprintln!(
                "Warning: Hub did not shut down within {}s. It may still be running.",
                HUB_EXIT_GRACE.as_secs()
            );
            eprintln!("  Run `loopal --list-hubs` to verify.");
        }
    }
    Ok(Some(
        outcome.session_id.unwrap_or(handshake.root_session_id),
    ))
}
