use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload, UiCapabilities, UiCapability};
use tokio::sync::{Mutex, Notify, mpsc};

use super::start_tcp_ui_io;
use crate::Hub;

struct LaggedResyncFailureTransport {
    closed: AtomicBool,
    first_event: AtomicBool,
    first_event_started: Notify,
    release_first_event: Notify,
}

#[async_trait]
impl Transport for LaggedResyncFailureTransport {
    async fn send(&self, data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        let value: serde_json::Value = serde_json::from_slice(data)
            .map_err(|error| loopal_error::LoopalError::Other(error.to_string()))?;
        match value["method"].as_str() {
            Some(method) if method == methods::VIEW_RESYNC_REQUIRED.name => {
                Err(loopal_error::LoopalError::Other("resync failed".into()))
            }
            Some(method)
                if method == methods::AGENT_EVENT.name
                    && !self.first_event.swap(true, Ordering::SeqCst) =>
            {
                self.first_event_started.notify_one();
                self.release_first_event.notified().await;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        pending().await
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

#[path = "tcp_ui_io_tests/lease_cleanup.rs"]
mod lease_cleanup;
#[path = "tcp_ui_io_tests/resync.rs"]
mod resync;
