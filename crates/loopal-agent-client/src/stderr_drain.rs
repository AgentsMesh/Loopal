use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::warn;

/// Consume lines from a child process stderr pipe and forward to tracing.
///
/// Prevents child stderr output from corrupting the TUI terminal.
/// Runs as a spawned task — exits naturally when the pipe closes (child exits).
pub async fn drain_to_tracing(stderr: impl AsyncRead + Unpin) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            warn!("agent stderr: {trimmed}");
        }
    }
}
