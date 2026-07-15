//! ACP mode — IDE integration via Hub.
//!
//! Flow: Start Hub → spawn root agent → connect ACP via UiSession.

use tracing::info;

use loopal_agent_hub::UiSession;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting in ACP mode (Hub-backed)");

    let ctx = super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config, None).await?;

    // Connect ACP as UI client (subscribes to events) BEFORE starting
    // the broadcast forwarder, so early events from agent boot do not
    // leak into the ether between broadcast-start and subscribe.
    let ui_session = UiSession::connect(ctx.hub.clone(), "acp").await;
    info!("ACP connected to Hub as UI client");

    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);

    // Run ACP adapter
    let result = loopal_acp::run_acp(ui_session).await;

    info!("shutting down agent process");
    let _ = ctx.agent_proc.shutdown().await;

    result
}
