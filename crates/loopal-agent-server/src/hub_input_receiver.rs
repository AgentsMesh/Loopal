use loopal_runtime::agent_input::AgentInput;
use tokio_util::sync::CancellationToken;

pub(crate) struct HubInputReceiver {
    input_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<AgentInput>>,
    interrupt_rx: tokio::sync::Mutex<tokio::sync::watch::Receiver<u64>>,
    shutdown: CancellationToken,
}

impl HubInputReceiver {
    pub fn new(
        input_rx: tokio::sync::mpsc::Receiver<AgentInput>,
        interrupt_rx: tokio::sync::watch::Receiver<u64>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            input_rx: tokio::sync::Mutex::new(input_rx),
            interrupt_rx: tokio::sync::Mutex::new(interrupt_rx),
            shutdown,
        }
    }

    pub async fn next(&self) -> Option<AgentInput> {
        let mut rx = self.input_rx.lock().await;
        let mut interrupt_rx = self.interrupt_rx.lock().await;
        // reason: clear stale interrupt token from previous turn so changed()
        // doesn't fire immediately on re-entry.
        interrupt_rx.borrow_and_update();
        tokio::select! {
            biased;
            // Session shutdown is level-triggered: a turn cancellation cannot
            // consume it before the runtime returns to the idle receive.
            _ = self.shutdown.cancelled() => None,
            msg = rx.recv() => msg,
            _ = interrupt_rx.changed() => None,
        }
    }

    pub async fn try_next(&self) -> Result<AgentInput, tokio::sync::mpsc::error::TryRecvError> {
        if self.shutdown.is_cancelled() {
            return Err(tokio::sync::mpsc::error::TryRecvError::Disconnected);
        }
        self.input_rx.lock().await.try_recv()
    }

    pub async fn drain(&self) -> Vec<AgentInput> {
        let mut rx = self.input_rx.lock().await;
        let mut inputs = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            inputs.push(msg);
        }
        inputs
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn shutdown_remains_observable_after_turn_teardown() {
        let (_input_tx, input_rx) = tokio::sync::mpsc::channel(1);
        let (_interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(0);
        let shutdown = CancellationToken::new();
        shutdown.cancel();
        let input = HubInputReceiver::new(input_rx, interrupt_rx, shutdown);

        let result = tokio::time::timeout(Duration::from_secs(1), input.next())
            .await
            .expect("pre-existing shutdown must wake the next idle receive");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn try_next_observes_shutdown_before_queued_input() {
        let (input_tx, input_rx) = tokio::sync::mpsc::channel(1);
        let (_interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(0);
        let shutdown = CancellationToken::new();
        input_tx
            .send(AgentInput::Message(loopal_protocol::Envelope::new(
                loopal_protocol::MessageSource::Human,
                "main",
                "must not run",
            )))
            .await
            .unwrap();
        shutdown.cancel();
        let input = HubInputReceiver::new(input_rx, interrupt_rx, shutdown);

        assert!(matches!(
            input.try_next().await,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
        ));
    }
}
