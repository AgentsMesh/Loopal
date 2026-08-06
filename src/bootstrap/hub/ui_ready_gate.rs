use std::time::Duration;

use loopal_protocol::UiCapabilities;

use super::typestate::RootPending;

const UI_READY_DEADLINE: Duration = Duration::from_secs(15);

pub async fn wait_for_interactive_ui(prepared: &RootPending) -> anyhow::Result<()> {
    let snapshot = loopal_agent_hub::wait_for_ui_capabilities(
        prepared.hub(),
        UiCapabilities::ALL,
        UI_READY_DEADLINE,
    )
    .await?;
    tracing::info!(
        generation = snapshot.generation,
        ?snapshot.capabilities,
        "interactive UI ready; releasing root startup gate"
    );
    Ok(())
}
