//! Thread-safe ring buffer for capturing the last N lines of command output.
//!
//! Used by `exec_command_streaming` to expose real-time stdout/stderr tail
//! to the progress reporter without buffering the entire output in memory.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Ring buffer holding the last `max_lines` of output.
///
/// Thread-safe: push from the reader task, snapshot from the progress reporter.
pub struct OutputTail {
    lines: Mutex<VecDeque<String>>,
    max_lines: usize,
}

impl OutputTail {
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Mutex::new(VecDeque::with_capacity(max_lines + 1)),
            max_lines,
        }
    }

    /// Append a line. If over capacity, oldest line is discarded.
    pub fn push_line(&self, line: String) {
        let mut buf = self.lines.lock().unwrap();
        if buf.len() >= self.max_lines {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    /// Return current tail lines joined by newline.
    pub fn snapshot(&self) -> String {
        let buf = self.lines.lock().unwrap();
        buf.iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
