//! In-memory duplex transport for testing (no network I/O).
//!
//! Uses two tokio::io::duplex channels cross-connected:
//! A writes to pipe1 → B reads from pipe1, B writes to pipe2 → A reads from pipe2.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_error::LoopalError;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::Mutex;

use crate::transport::Transport;

/// Create a pair of connected in-memory transports.
pub fn duplex_pair() -> (Arc<dyn Transport>, Arc<dyn Transport>) {
    // Two pipes: a→b direction and b→a direction
    let (a_write, b_read) = tokio::io::duplex(1024 * 1024);
    let (b_write, a_read) = tokio::io::duplex(1024 * 1024);
    let connected = Arc::new(AtomicBool::new(true));
    (
        Arc::new(DuplexTransport {
            reader: Mutex::new(BufReader::new(a_read)),
            writer: Mutex::new(a_write),
            connected: connected.clone(),
        }),
        Arc::new(DuplexTransport {
            reader: Mutex::new(BufReader::new(b_read)),
            writer: Mutex::new(b_write),
            connected,
        }),
    )
}

struct DuplexTransport {
    reader: Mutex<BufReader<DuplexStream>>,
    writer: Mutex<DuplexStream>,
    connected: Arc<AtomicBool>,
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&self, data: &[u8]) -> Result<(), LoopalError> {
        let mut writer = self.writer.lock().await;
        writer
            .write_all(data)
            .await
            .map_err(|e| LoopalError::Ipc(e.to_string()))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| LoopalError::Ipc(e.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|e| LoopalError::Ipc(e.to_string()))
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, LoopalError> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                self.connected.store(false, Ordering::Relaxed);
                Ok(None)
            }
            Ok(_) => {
                if line.ends_with('\n') {
                    line.pop();
                }
                Ok(Some(line.into_bytes()))
            }
            Err(e) => {
                self.connected.store(false, Ordering::Relaxed);
                Err(LoopalError::Ipc(e.to_string()))
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}
