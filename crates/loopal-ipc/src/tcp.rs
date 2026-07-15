//! TCP-based transport: localhost connections for multi-client IPC.
//!
//! Uses newline-delimited framing over a TCP stream, identical to
//! `StdioTransport`. Each message is a single JSON line terminated by `\n`.

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_error::LoopalError;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
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
        crate::frame::validate_outgoing_frame(data)?;
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
        let result = crate::frame::read_frame(&mut *reader).await;
        if result.as_ref().is_err() || matches!(&result, Ok(None)) {
            self.connected.store(false, Ordering::Release);
        }
        result
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
            tracing::warn!("TCP transport close: {e}");
        }
        self.connected.store(false, Ordering::Release);
    }
}
