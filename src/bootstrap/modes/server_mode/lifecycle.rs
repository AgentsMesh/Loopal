use tracing::info;

use loopal_agent_hub::UiSession;
use loopal_protocol::UiCapabilities;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!(
        "starting in server mode (ephemeral={})",
        cli.child.ephemeral
    );

    let prepared = super::super::hub_bootstrap::prepare_hub_and_agent(cli, cwd, config).await?;
    // Install the responder lease before consuming the typestate that starts the root agent.
    let capabilities = UiCapabilities {
        permission: true,
        question: true,
        plan_approval: true,
    };
    let ui_session = UiSession::connect(prepared.hub().clone(), "server", capabilities).await;
    info!("server client connected to Hub");
    let ctx =
        super::super::hub_bootstrap::start_prepared_hub_and_agent(prepared, cli, cwd, config, None)
            .await?;

    let output = super::consume_events(ui_session.event_rx, ui_session.client.clone()).await;
    if !output.is_empty() {
        println!("{output}");
    }

    info!("server mode complete, shutting down");
    ctx.shutdown().await
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
