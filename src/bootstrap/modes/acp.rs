//! ACP mode — IDE integration via Hub.
//!
//! Flow: prepare Hub → connect ACP via UiSession → start root agent.

use tracing::info;

use loopal_agent_hub::UiSession;
use loopal_protocol::UiCapabilities;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting in ACP mode (Hub-backed)");

    let prepared = super::hub_bootstrap::prepare_hub_and_agent(cli, cwd, config).await?;

    // Connect ACP as UI client (subscribes to events) BEFORE starting
    // the broadcast forwarder, so early events from agent boot do not
    // leak into the ether between broadcast-start and subscribe.
    let capabilities = UiCapabilities {
        permission: true,
        question: true,
        plan_approval: false,
    };
    let ui_session = UiSession::connect(prepared.hub().clone(), "acp", capabilities).await;
    info!("ACP connected to Hub as UI client");
    let ctx = super::hub_bootstrap::start_prepared_hub_and_agent(prepared, cli, cwd, config, None)
        .await?;

    // Run ACP adapter
    let result = loopal_acp::run_acp(ui_session).await;

    info!("shutting down agent process");
    let _ = ctx.agent_proc.shutdown().await;

    result
}
