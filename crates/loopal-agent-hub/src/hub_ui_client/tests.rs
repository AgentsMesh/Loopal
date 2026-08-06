use std::future::pending;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_ipc::Transport;

use super::*;

struct NoResponseTransport {
    sends: AtomicUsize,
}

#[async_trait]
impl Transport for NoResponseTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        pending().await
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn close(&self) {}
}

#[tokio::test]
async fn interrupt_returns_when_hub_never_responds() {
    let transport = Arc::new(NoResponseTransport {
        sends: AtomicUsize::new(0),
    });
    let (connection, _incoming) = Connection::new(transport.clone()).into_listening();
    let client = HubClient::new(connection);

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        client.interrupt_target("main"),
    )
    .await
    .expect("interrupt must have an outer deadline");

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while transport.sends.load(Ordering::Acquire) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timed-out request must notify the Hub of cancellation");
}
