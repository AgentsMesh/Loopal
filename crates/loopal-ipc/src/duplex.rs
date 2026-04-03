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
///
/// Each side has its own independent `connected` flag so that closing
/// one side does not prevent the other side from closing its writer.
pub fn duplex_pair() -> (Arc<dyn Transport>, Arc<dyn Transport>) {
    // Two pipes: a→b direction and b→a direction
    let (a_write, b_read) = tokio::io::duplex(1024 * 1024);
    let (b_write, a_read) = tokio::io::duplex(1024 * 1024);
    (
        Arc::new(DuplexTransport {
            reader: Mutex::new(BufReader::new(a_read)),
            writer: Mutex::new(a_write),
            connected: AtomicBool::new(true),
        }),
        Arc::new(DuplexTransport {
            reader: Mutex::new(BufReader::new(b_read)),
            writer: Mutex::new(b_write),
            connected: AtomicBool::new(true),
        }),
    )
}

struct DuplexTransport {
    reader: Mutex<BufReader<DuplexStream>>,
    writer: Mutex<DuplexStream>,
    connected: AtomicBool,
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&self, data: &[u8]) -> Result<(), LoopalError> {
        let mut writer = self.writer.lock().await;
        let result = async {
            writer.write_all(data).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await
        }
        .await;
        if let Err(ref e) = result {
            self.connected.store(false, Ordering::Release);
            return Err(LoopalError::Ipc(format!("write failed: {e}")));
        }
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, LoopalError> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                self.connected.store(false, Ordering::Release);
                Ok(None)
            }
            Ok(_) => {
                if line.ends_with('\n') {
                    line.pop();
                }
                Ok(Some(line.into_bytes()))
            }
            Err(e) => {
                self.connected.store(false, Ordering::Release);
                Err(LoopalError::Ipc(e.to_string()))
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    async fn close(&self) {
        if !self.is_connected() {
            return;
        }
        let mut w = self.writer.lock().await;
        if let Err(e) = w.shutdown().await {
            tracing::warn!("duplex transport close: {e}");
        }
        self.connected.store(false, Ordering::Release);
    }
}
