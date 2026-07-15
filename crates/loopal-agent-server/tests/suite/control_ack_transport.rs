use std::sync::Arc;

use loopal_error::LoopalError;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use tokio::sync::Notify;

pub struct ControlAckTransport {
    inner: Arc<dyn Transport>,
    sent: Arc<Notify>,
}

impl ControlAckTransport {
    pub fn wrap(inner: Arc<dyn Transport>) -> (Arc<dyn Transport>, Arc<Notify>) {
        let sent = Arc::new(Notify::new());
        let transport = Self {
            inner,
            sent: sent.clone(),
        };
        (Arc::new(transport), sent)
    }
}

#[async_trait::async_trait]
impl Transport for ControlAckTransport {
    async fn send(&self, data: &[u8]) -> Result<(), LoopalError> {
        self.inner.send(data).await?;
        let is_control = serde_json::from_slice::<serde_json::Value>(data)
            .ok()
            .and_then(|value| value["method"].as_str().map(str::to_owned))
            .is_some_and(|method| method == methods::AGENT_CONTROL.name);
        if is_control {
            self.sent.notify_one();
        }
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, LoopalError> {
        self.inner.recv().await
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    async fn close(&self) {
        self.inner.close().await;
    }
}
