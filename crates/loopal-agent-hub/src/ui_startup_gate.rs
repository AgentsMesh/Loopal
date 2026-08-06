use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_protocol::UiCapabilities;

use crate::{Hub, UiCapabilitySnapshot};

/// Wait until live UI leases collectively cover `required`.
///
/// The receiver is subscribed while holding the Hub lock, so a registration
/// cannot land between the initial snapshot and the watch subscription.
pub async fn wait_for_ui_capabilities(
    hub: &Arc<Mutex<Hub>>,
    required: UiCapabilities,
    deadline: Duration,
) -> anyhow::Result<UiCapabilitySnapshot> {
    let mut state = hub.lock().await.ui.subscribe_capabilities();
    let wait = async {
        loop {
            let snapshot = *state.borrow_and_update();
            if covers(snapshot.capabilities, required) {
                return Ok(snapshot);
            }
            state
                .changed()
                .await
                .map_err(|_| anyhow::anyhow!("UI capability lifecycle closed during startup"))?;
        }
    };
    tokio::time::timeout(deadline, wait).await.map_err(|_| {
        anyhow::anyhow!(
            "required UI capabilities were not registered within {:.1}s: {:?}",
            deadline.as_secs_f64(),
            required
        )
    })?
}

fn covers(actual: UiCapabilities, required: UiCapabilities) -> bool {
    (!required.permission || actual.permission)
        && (!required.question || actual.question)
        && (!required.plan_approval || actual.plan_approval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiSession;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn lease_registration_advances_gate_snapshot() {
        let (event_tx, _event_rx) = mpsc::channel(4);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let waiter = tokio::spawn({
            let hub = hub.clone();
            async move {
                wait_for_ui_capabilities(&hub, UiCapabilities::ALL, Duration::from_secs(1)).await
            }
        });

        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        let _ui = UiSession::connect(hub, "startup-ui", UiCapabilities::ALL).await;

        let snapshot = waiter.await.unwrap().unwrap();
        assert!(snapshot.generation > 0);
        assert_eq!(snapshot.capabilities, UiCapabilities::ALL);
    }

    #[tokio::test]
    async fn missing_capability_has_a_bounded_failure() {
        let (event_tx, _event_rx) = mpsc::channel(4);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let error = wait_for_ui_capabilities(&hub, UiCapabilities::ALL, Duration::from_millis(10))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("not registered"));
    }
}
