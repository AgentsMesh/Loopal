use std::sync::Arc;

use loopal_tool_api::OutputTail;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

use crate::log_writer::{LineSink, LogWriter};

const READ_CHUNK_BYTES: usize = 8 * 1024;
const MAX_PREVIEW_LINE_BYTES: usize = 64 * 1024;
const TRUNCATED_LINE_SUFFIX: &str = " [... line truncated ...]";

pub(crate) async fn capture<R: AsyncRead + Unpin>(
    mut reader: R,
    writer: Arc<LogWriter>,
    sink: LineSink,
    progress_tail: Option<Arc<OutputTail>>,
) {
    let mut input = Zeroizing::new(vec![0u8; READ_CHUNK_BYTES]);
    let mut preview = PreviewLine::new();
    let mut file_line_start = true;
    loop {
        let (safe, eof) = match reader.read(&mut input).await {
            Ok(0) => (Vec::new(), true),
            Ok(read) => (input[..read].to_vec(), false),
            Err(_) => return,
        };
        if !safe.is_empty() {
            if !write_safe(&writer, &sink, &safe, &mut file_line_start).await {
                return;
            }
            preview.absorb(&safe, &sink, &progress_tail);
        }
        if eof {
            preview.finish(&sink, &progress_tail);
            return;
        }
    }
}

async fn write_safe(
    writer: &LogWriter,
    sink: &LineSink,
    safe: &[u8],
    line_start: &mut bool,
) -> bool {
    let mut file = writer.lock().await;
    if matches!(sink, LineSink::Stdout(_)) {
        return file.write_all(safe).await.is_ok() && file.flush().await.is_ok();
    }
    for segment in safe.split_inclusive(|byte| *byte == b'\n') {
        if *line_start && file.write_all(b"[err] ").await.is_err() {
            return false;
        }
        if file.write_all(segment).await.is_err() {
            return false;
        }
        *line_start = segment.ends_with(b"\n");
    }
    file.flush().await.is_ok()
}

struct PreviewLine {
    bytes: Vec<u8>,
    truncated: bool,
}

impl PreviewLine {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            truncated: false,
        }
    }

    fn absorb(&mut self, safe: &[u8], sink: &LineSink, progress_tail: &Option<Arc<OutputTail>>) {
        for byte in safe {
            if *byte == b'\n' {
                self.emit(true, sink, progress_tail);
            } else if self.bytes.len() < MAX_PREVIEW_LINE_BYTES {
                self.bytes.push(*byte);
            } else {
                self.truncated = true;
            }
        }
    }

    fn finish(&mut self, sink: &LineSink, progress_tail: &Option<Arc<OutputTail>>) {
        if !self.bytes.is_empty() || self.truncated {
            self.emit(false, sink, progress_tail);
        }
    }

    fn emit(&mut self, newline: bool, sink: &LineSink, progress_tail: &Option<Arc<OutputTail>>) {
        let mut text = String::from_utf8_lossy(&self.bytes).into_owned();
        if self.truncated {
            text.push_str(TRUNCATED_LINE_SUFFIX);
        }
        if let Some(tail) = progress_tail {
            tail.push_line(text.clone());
        }
        match sink {
            LineSink::Stdout(head_tail) => head_tail.push_line(text),
            LineSink::Stderr(stderr) => {
                if newline {
                    text.push('\n');
                }
                stderr.lock().push_str(&text);
            }
        }
        self.bytes.clear();
        self.truncated = false;
    }
}
