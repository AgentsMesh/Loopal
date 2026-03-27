//! TCP-based transport: localhost connections for multi-client IPC.
//!
//! Uses newline-delimited framing over a TCP stream, identical to
//! `StdioTransport`. Each message is a single JSON line terminated by `\n`.

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_error::LoopalError;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use crate::transport::Transport;

/// Newline-delimited transport over a TCP stream.
///
/// Splits the `TcpStream` into independent read/write halves so that
/// send and recv can proceed concurrently without contention.
pub struct TcpTransport {
    reader: Mutex<BufReader<OwnedReadHalf>>,
    writer: Mutex<BufWriter<OwnedWriteHalf>>,
    connected: AtomicBool,
}

impl TcpTransport {
    /// Wrap an established `TcpStream` as a transport.
    pub fn new(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: Mutex::new(BufReader::new(read_half)),
            writer: Mutex::new(BufWriter::new(write_half)),
            connected: AtomicBool::new(true),
        }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn send(&self, data: &[u8]) -> Result<(), LoopalError> {
        let mut w = self.writer.lock().await;
        let result = async {
            w.write_all(data).await?;
            w.write_all(b"\n").await?;
            w.flush().await
        }
        .await;
        if let Err(ref e) = result {
            tracing::warn!("TCP transport: write failed, disconnecting: {e}");
            self.connected.store(false, Ordering::Release);
            return Err(LoopalError::Ipc(format!("write failed: {e}")));
        }
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, LoopalError> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| LoopalError::Ipc(format!("read failed: {e}")))?;
            if n == 0 {
                self.connected.store(false, Ordering::Release);
                return Ok(None); // EOF / peer closed
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.as_bytes().to_vec()));
            }
            // Skip blank lines
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
}
