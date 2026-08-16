use zeroize::Zeroizing;

use crate::process_capture_buffers::{CappedText, HeadTail, Tail};
use crate::shell_spawn::{HEAD_LINES, TAIL_LINES};

const MAX_PREVIEW_LINE_BYTES: usize = 64 * 1024;
const TRUNCATED_LINE_SUFFIX: &str = " [... line truncated ...]";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureSource {
    Stdout,
    Stderr,
}

pub(crate) struct RenderedPreviews {
    pub stdout: Zeroizing<String>,
    pub stderr: Zeroizing<String>,
    pub progress: Zeroizing<String>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

pub(crate) struct ProcessPreviews {
    stdout_line: PreviewLine,
    stderr_line: PreviewLine,
    stdout: HeadTail,
    stderr: CappedText,
    progress: Tail,
}

impl ProcessPreviews {
    pub fn new(progress_lines: usize) -> Self {
        Self {
            stdout_line: PreviewLine::new(),
            stderr_line: PreviewLine::new(),
            stdout: HeadTail::new(HEAD_LINES, TAIL_LINES),
            stderr: CappedText::new(),
            progress: Tail::new(progress_lines),
        }
    }

    pub fn absorb(&mut self, source: CaptureSource, bytes: &[u8]) {
        let (line, stdout) = match source {
            CaptureSource::Stdout => (&mut self.stdout_line, true),
            CaptureSource::Stderr => (&mut self.stderr_line, false),
        };
        let mut emitted = Vec::new();
        for byte in bytes {
            if *byte == b'\n' {
                if let Some(text) = line.emit() {
                    emitted.push((text, true));
                }
            } else {
                line.push(*byte);
            }
        }
        for (text, newline) in emitted {
            self.push_line(stdout, text, newline);
        }
    }

    pub fn finish(&mut self, source: CaptureSource) {
        let (line, stdout) = match source {
            CaptureSource::Stdout => (&mut self.stdout_line, true),
            CaptureSource::Stderr => (&mut self.stderr_line, false),
        };
        if let Some(text) = line.emit() {
            self.push_line(stdout, text, false);
        }
    }

    pub fn render(&self) -> RenderedPreviews {
        RenderedPreviews {
            stdout: self.stdout.render(),
            stderr: self.stderr.snapshot(),
            progress: self.progress.render(),
            stdout_truncated: self.stdout.was_truncated(),
            stderr_truncated: self.stderr.truncated,
        }
    }

    fn push_line(&mut self, stdout: bool, mut text: Zeroizing<String>, newline: bool) {
        self.progress.push(text.clone());
        if stdout {
            self.stdout.push(text);
        } else {
            if newline {
                text.push('\n');
            }
            self.stderr.push(&text);
        }
    }
}

struct PreviewLine {
    bytes: Zeroizing<Vec<u8>>,
    truncated: bool,
}

impl PreviewLine {
    fn new() -> Self {
        Self {
            bytes: Zeroizing::new(Vec::new()),
            truncated: false,
        }
    }

    fn push(&mut self, byte: u8) {
        if self.bytes.len() < MAX_PREVIEW_LINE_BYTES {
            self.bytes.push(byte);
        } else {
            self.truncated = true;
        }
    }

    fn emit(&mut self) -> Option<Zeroizing<String>> {
        if self.bytes.is_empty() && !self.truncated {
            return None;
        }
        let decoded = std::str::from_utf8(&self.bytes).expect("capture decoder emits valid UTF-8");
        let mut text = Zeroizing::new(String::with_capacity(
            decoded.len() + TRUNCATED_LINE_SUFFIX.len(),
        ));
        text.push_str(decoded);
        if self.truncated {
            text.push_str(TRUNCATED_LINE_SUFFIX);
        }
        self.bytes.clear();
        self.truncated = false;
        Some(text)
    }
}
