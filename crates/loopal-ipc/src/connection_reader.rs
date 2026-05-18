use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::connection::{Incoming, PendingMap};
use crate::jsonrpc::{self, IncomingMessage};
use crate::rpc_error::RpcError;
use crate::transport::Transport;

pub(crate) fn spawn_reader_loop(
    transport: Arc<dyn Transport>,
    pending: PendingMap,
) -> mpsc::Receiver<Incoming> {
    let (tx, rx) = mpsc::channel::<Incoming>(256);

    tokio::spawn(async move {
        debug!("IPC reader loop started");
        loop {
            let data = match transport.recv().await {
                Ok(Some(data)) => data,
                Ok(None) => {
                    debug!("IPC connection: EOF, reader loop exiting");
                    break;
                }
                Err(e) => {
                    warn!("IPC connection read error: {e}");
                    break;
                }
            };

            let Some(msg) = jsonrpc::parse_message(&data) else {
                warn!("IPC connection: malformed message, skipping");
                continue;
            };

            if !dispatch_message(msg, &tx, &pending).await {
                break;
            }
        }

        let mut map = pending.lock().await;
        if !map.is_empty() {
            warn!(
                "IPC reader: dropping {} pending requests on exit",
                map.len()
            );
            map.clear();
        }
    });

    rx
}

async fn dispatch_message(
    msg: IncomingMessage,
    tx: &mpsc::Sender<Incoming>,
    pending: &PendingMap,
) -> bool {
    match msg {
        IncomingMessage::Response { id, result, error } => {
            let outcome = if let Some(err) = error {
                Err(RpcError::Remote {
                    code: err.code,
                    message: err.message,
                    data: err.data,
                })
            } else {
                Ok(result.unwrap_or(Value::Null))
            };
            if let Some(sender) = pending.lock().await.remove(&id) {
                let _ = sender.send(outcome);
            }
            true
        }
        IncomingMessage::Request { id, method, params } => {
            if tx
                .send(Incoming::Request { id, method, params })
                .await
                .is_err()
            {
                debug!("IPC reader: incoming channel closed, exiting");
                return false;
            }
            true
        }
        IncomingMessage::Notification { method, params } => {
            if tx
                .send(Incoming::Notification { method, params })
                .await
                .is_err()
            {
                debug!("IPC reader: incoming channel closed, exiting");
                return false;
            }
            true
        }
    }
}
