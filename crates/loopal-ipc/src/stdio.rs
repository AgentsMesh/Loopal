//! Stdio-based transport: most portable, works on all platforms.
//!
//! Uses newline-delimited framing over stdin/stdout, identical to the pattern
//! used by MCP servers and the existing ACP protocol. Each message is a single
//! JSON line terminated by `\n`.

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_error::LoopalError;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;

use crate::transport::Transport;

/// Newline-delimited transport over stdin/stdout (or arbitrary async streams).
pub struct StdioTransport {
    reader: Mutex<Box<dyn tokio::io::AsyncBufRead + Unpin + Send>>,
    writer: Mutex<BufWriter<Box<dyn AsyncWrite + Unpin + Send>>>,
    connected: AtomicBool,
}

impl StdioTransport {
    /// Create a transport using process stdin/stdout.
    pub fn from_std() -> Self {
        Self::new(
            Box::new(BufReader::new(tokio::io::stdin())),
            Box::new(tokio::io::stdout()),
        )
    }

    /// Create a transport from arbitrary async reader/writer (for testing).
    pub fn new(
        reader: Box<dyn tokio::io::AsyncBufRead + Unpin + Send>,
        writer: Box<dyn AsyncWrite + Unpin + Send>,
    ) -> Self {
        Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(BufWriter::new(writer)),
            connected: AtomicBool::new(true),
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&self, data: &[u8]) -> Result<(), LoopalError> {
        let mut w = self.writer.lock().await;
        let result = async {
            w.write_all(data).await?;
            w.write_all(b"\n").await?;
            w.flush().await
        }
        .await;
        if let Err(ref e) = result {
            tracing::warn!("IPC transport: write failed, disconnecting: {e}");
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
                return Ok(None); // EOF
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
