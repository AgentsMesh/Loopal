use loopal_runtime::agent_input::AgentInput;

pub(crate) struct HubInputReceiver {
    input_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<AgentInput>>,
    interrupt_rx: tokio::sync::Mutex<tokio::sync::watch::Receiver<u64>>,
}

impl HubInputReceiver {
    pub fn new(
        input_rx: tokio::sync::mpsc::Receiver<AgentInput>,
        interrupt_rx: tokio::sync::watch::Receiver<u64>,
    ) -> Self {
        Self {
            input_rx: tokio::sync::Mutex::new(input_rx),
            interrupt_rx: tokio::sync::Mutex::new(interrupt_rx),
        }
    }

    pub async fn next(&self) -> Option<AgentInput> {
        let mut rx = self.input_rx.lock().await;
        let mut interrupt_rx = self.interrupt_rx.lock().await;
        // reason: clear stale interrupt token from previous turn so changed()
        // doesn't fire immediately on re-entry.
        interrupt_rx.borrow_and_update();
        tokio::select! {
            msg = rx.recv() => msg,
            _ = interrupt_rx.changed() => None,
        }
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
