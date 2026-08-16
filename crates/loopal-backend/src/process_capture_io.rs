use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::ToolIoError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::log_writer::LogWriter;

#[async_trait]
pub(crate) trait CaptureSink: Send + Sync {
    async fn write_all(&self, bytes: &[u8]) -> io::Result<()>;
    async fn flush(&self) -> io::Result<()>;
}

pub(crate) struct LogCaptureSink(Arc<LogWriter>);

impl LogCaptureSink {
    pub(crate) fn new(writer: Arc<LogWriter>) -> Self {
        Self(writer)
    }
}

#[async_trait]
impl CaptureSink for LogCaptureSink {
    async fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        self.0.lock().await.write_all(bytes).await
    }

    async fn flush(&self) -> io::Result<()> {
        self.0.lock().await.flush().await
    }
}

pub(crate) async fn persist(sink: &dyn CaptureSink, safe: &[u8]) -> Result<(), ToolIoError> {
    if safe.is_empty() {
        return Ok(());
    }
    sink.write_all(safe)
        .await
        .map_err(|_| capture_error("log write failed"))?;
    sink.flush()
        .await
        .map_err(|_| capture_error("log flush failed"))
}

pub(crate) async fn read_pipe<R: AsyncRead + Unpin>(
    pipe: &mut Option<R>,
    buffer: &mut [u8],
) -> Result<usize, ToolIoError> {
    pipe.as_mut()
        .expect("select guard requires a pipe")
        .read(buffer)
        .await
        .map_err(|_| capture_error("pipe read failed"))
}

pub(crate) fn capture_error(stage: &str) -> ToolIoError {
    ToolIoError::ExecFailed(format!("process output capture {stage}"))
}
