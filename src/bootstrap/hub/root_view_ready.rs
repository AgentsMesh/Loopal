pub async fn wait(
    hub: &std::sync::Arc<tokio::sync::Mutex<loopal_agent_hub::Hub>>,
) -> anyhow::Result<()> {
    let timeout = std::env::var("LOOPAL_TEST_ROOT_VIEW_TIMEOUT_MS")
        .ok()
        .filter(|_| std::env::var_os("LOOPAL_TEST_PROVIDER").is_some())
        .and_then(|value| value.parse().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or_else(|| std::time::Duration::from_secs(2));
    if timeout.is_zero() {
        return Err(not_ready());
    }
    let ready = async {
        loop {
            let view = {
                let hub = hub.lock().await;
                hub.registry.agent_view(loopal_protocol::ROOT_AGENT_NAME)
            };
            if let Some(view) = view {
                let view = view.lock().await;
                if view.rev() > 0
                    && view.state().agent.observable.status
                        != loopal_protocol::AgentStatus::Starting
                {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(timeout, ready)
        .await
        .map_err(|_| not_ready())
}

fn not_ready() -> anyhow::Error {
    anyhow::anyhow!("root ViewState stayed Starting before Desktop READY")
}
