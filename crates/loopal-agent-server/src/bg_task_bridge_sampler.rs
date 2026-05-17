use std::path::PathBuf;
use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::traits::EventEmitter;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

// reason: cap per-tick read so a 100MB log spike between ticks doesn't allocate
// a single huge buffer — remainder is picked up next tick.
const MAX_READ_PER_TICK: u64 = 64 * 1024;

// reason: tail the log file by seek + bounded read. LogWriter is unbuffered,
// so every line is on disk by the time the writer.lock() drop returns —
// sampler sees fresh bytes without explicit flush. UI sees stdout + stderr-
// as-[err]-prefixed in the order they were written. Partial UTF-8 sequences
// at the tail are stashed in `carry` and prepended to the next read so
// multi-byte chars never surface as `�` replacement.
pub(super) async fn run_file_sampler(
    task_id: String,
    log_path: PathBuf,
    emitter: Box<dyn EventEmitter>,
    sample_interval: Duration,
) {
    let mut last_offset: u64 = 0;
    let mut carry: Vec<u8> = Vec::new();
    let mut interval = tokio::time::interval(sample_interval.max(Duration::from_millis(1)));
    interval.tick().await;
    loop {
        interval.tick().await;
        let Ok(meta) = tokio::fs::metadata(&log_path).await else {
            continue;
        };
        if meta.len() <= last_offset {
            continue;
        }
        let Ok(mut file) = tokio::fs::File::open(&log_path).await else {
            continue;
        };
        if file.seek(SeekFrom::Start(last_offset)).await.is_err() {
            continue;
        }
        let mut buf = Vec::new();
        let n = file
            .take(MAX_READ_PER_TICK)
            .read_to_end(&mut buf)
            .await
            .unwrap_or(0);
        last_offset += n as u64;
        let mut combined = std::mem::take(&mut carry);
        combined.extend_from_slice(&buf);
        let boundary = utf8_safe_split(&combined);
        carry = combined.split_off(boundary);
        if combined.is_empty() {
            continue;
        }
        let delta = match String::from_utf8(combined) {
            Ok(s) => s,
            Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
        };
        if let Err(e) = emitter
            .emit(AgentEventPayload::BgTaskOutput {
                id: task_id.clone(),
                output_delta: delta,
            })
            .await
        {
            tracing::warn!(error = %e, label = "BgTaskOutput", "failed to emit");
        }
    }
}

// reason: returns the largest prefix length of `bytes` that is valid UTF-8.
// Bytes after are the (partial) start of a multi-byte sequence and must
// carry over to the next read.
pub(super) fn utf8_safe_split(bytes: &[u8]) -> usize {
    match std::str::from_utf8(bytes) {
        Ok(_) => bytes.len(),
        Err(e) => e.valid_up_to(),
    }
}

#[cfg(test)]
mod tests {
    use super::utf8_safe_split;

    #[test]
    fn ascii_only_keeps_full_slice() {
        let s = b"hello world";
        assert_eq!(utf8_safe_split(s), s.len());
    }

    #[test]
    fn complete_multibyte_keeps_full_slice() {
        let s = "中文".as_bytes();
        assert_eq!(utf8_safe_split(s), s.len());
    }

    #[test]
    fn truncated_multibyte_splits_before_partial() {
        let mut s = "中".as_bytes().to_vec();
        s.push(0xE4); // start of next "中" — incomplete
        let cut = utf8_safe_split(&s);
        assert_eq!(cut, 3); // first 中 (3 bytes) only
    }

    #[test]
    fn empty_slice() {
        assert_eq!(utf8_safe_split(&[]), 0);
    }
}
