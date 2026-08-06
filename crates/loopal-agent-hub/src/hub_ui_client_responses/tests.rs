use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::{Connection, Transport};

use crate::HubClient;

struct BlackholeTransport;

#[async_trait]
impl Transport for BlackholeTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        std::future::pending().await
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn close(&self) {}
}

#[tokio::test]
async fn interaction_response_does_not_wait_forever_for_transport_ack() {
    let transport: Arc<dyn Transport> = Arc::new(BlackholeTransport);
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let client = HubClient::new(connection);

    tokio::time::timeout(
        Duration::from_millis(500),
        client.respond_permission("main", "interaction-token", true),
    )
    .await
    .expect("UI interaction response must return after its bounded deadline");
}
