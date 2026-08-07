//! Reliable admission into the Hub's authoritative event stream.

use std::fmt;
use std::sync::Arc;

use loopal_protocol::AgentEvent;
use tokio::sync::{Notify, mpsc};

use crate::Hub;

/// Cloneable handle captured under the Hub lock. Creating an event delivery
/// from it is synchronous; waiting for bounded queue capacity is not.
#[derive(Clone)]
pub(crate) struct AuthoritativeEventSink {
    event_tx: mpsc::Sender<AgentEvent>,
    shutdown_signal: Arc<Notify>,
}

impl AuthoritativeEventSink {
    pub(crate) fn from_hub(hub: &Hub) -> Self {
        Self {
            event_tx: hub.registry.event_sender(),
            shutdown_signal: hub.shutdown_signal.clone(),
        }
    }

    pub(crate) fn prepare(&self, event: AgentEvent) -> PreparedAuthoritativeEvent {
        PreparedAuthoritativeEvent {
            event_tx: self.event_tx.clone(),
            shutdown_signal: self.shutdown_signal.clone(),
            event: Some(event),
        }
    }
}

/// An event prepared while the Hub lock is held and delivered afterwards.
///
/// `deliver` reserves queue capacity before taking ownership of the event. A
/// caller that polls it through a borrowed future can therefore cancel and
/// retry without losing the event. Production callers whose surrounding task
/// may be cancelled should run the entire post-admission coordinator in a
/// spawned task so the prepared event remains owned until it is admitted.
#[must_use = "authoritative events must be delivered after releasing the Hub lock"]
pub(crate) struct PreparedAuthoritativeEvent {
    event_tx: mpsc::Sender<AgentEvent>,
    shutdown_signal: Arc<Notify>,
    event: Option<AgentEvent>,
}

impl PreparedAuthoritativeEvent {
    pub(crate) fn from_hub(hub: &Hub, event: AgentEvent) -> Self {
        AuthoritativeEventSink::from_hub(hub).prepare(event)
    }

    /// Wait for bounded queue capacity without holding the Hub lock.
    ///
    /// A full queue is normal backpressure. A closed queue means the sole
    /// reducer/broadcast owner has disappeared, so the Hub is no longer able
    /// to maintain authoritative observable state and must shut down.
    pub(crate) async fn deliver(&mut self) -> Result<(), AuthoritativeEventQueueClosed> {
        if self.event.is_none() {
            return Ok(());
        }
        let permit = match self.event_tx.reserve().await {
            Ok(permit) => permit,
            Err(_) => {
                self.shutdown_signal.notify_one();
                return Err(AuthoritativeEventQueueClosed);
            }
        };
        let event = self
            .event
            .take()
            .expect("authoritative event disappeared after queue reservation");
        permit.send(event);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthoritativeEventQueueClosed;

impl fmt::Display for AuthoritativeEventQueueClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("authoritative Hub event queue closed")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use loopal_protocol::{AgentEvent, AgentEventPayload};
    use tokio::sync::{Mutex, mpsc};

    use super::*;

    #[tokio::test]
    async fn cancelled_backpressure_can_retry_the_same_prepared_event() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let mut delivery = {
            let hub = hub.lock().await;
            PreparedAuthoritativeEvent::from_hub(
                &hub,
                AgentEvent::root(AgentEventPayload::AwaitingInput),
            )
        };

        assert!(
            tokio::time::timeout(Duration::from_millis(10), delivery.deliver())
                .await
                .is_err(),
            "the first delivery must remain backpressured"
        );
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));

        delivery.deliver().await.unwrap();
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::AwaitingInput
        ));
    }

    #[tokio::test]
    async fn closed_queue_notifies_hub_shutdown_without_consuming_the_event() {
        let (event_tx, event_rx) = mpsc::channel(1);
        drop(event_rx);
        let hub = Hub::new(event_tx);
        let shutdown = hub.shutdown_signal.clone();
        let notified = shutdown.notified();
        let mut delivery = PreparedAuthoritativeEvent::from_hub(
            &hub,
            AgentEvent::root(AgentEventPayload::AwaitingInput),
        );

        assert_eq!(delivery.deliver().await, Err(AuthoritativeEventQueueClosed));
        tokio::time::timeout(Duration::from_millis(100), notified)
            .await
            .expect("closed authoritative queue must signal Hub shutdown");
        assert_eq!(
            delivery.deliver().await,
            Err(AuthoritativeEventQueueClosed),
            "a failed delivery remains retryable for diagnostics/ownership"
        );
    }
}
