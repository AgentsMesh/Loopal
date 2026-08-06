use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Listening};

pub(super) struct TransportLeaseGuard {
    conn: Arc<Connection<Listening>>,
}

impl TransportLeaseGuard {
    pub(super) fn new(conn: Arc<Connection<Listening>>) -> Self {
        Self { conn }
    }
}

impl Drop for TransportLeaseGuard {
    fn drop(&mut self) {
        if !self.conn.is_connected() {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("cannot close owned Hub UI transport outside a Tokio runtime");
            return;
        };
        let conn = self.conn.clone();
        runtime.spawn(async move {
            if tokio::time::timeout(Duration::from_secs(2), conn.close())
                .await
                .is_err()
            {
                tracing::warn!("timed out closing owned Hub UI transport");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HubClient;

    #[tokio::test]
    async fn dropping_owned_client_closes_transport() {
        let (client_transport, server_transport) = loopal_ipc::duplex_pair();
        let client_transport_ref = client_transport.clone();
        let (conn, _incoming) = Connection::new(client_transport).into_listening();
        let (_server, _server_incoming) = Connection::new(server_transport).into_listening();

        drop(HubClient::new_with_transport_lease(conn));

        tokio::time::timeout(Duration::from_secs(1), async {
            while client_transport_ref.is_connected() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("transport lease must close after owner drop");
    }
}
