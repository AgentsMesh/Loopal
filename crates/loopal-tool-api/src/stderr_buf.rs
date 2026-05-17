// reason: 8 KB stderr in-memory cap. Stderr is usually diagnostics (warnings /
// errors) — 8 KB ≈ 100-200 lines covers 95% of real-world cases. Beyond cap,
// we keep the most recent bytes (errors usually surface late) and signal the
// reader to consult the full log file.
pub const STDERR_CAP_BYTES: usize = 8 * 1024;

// reason: trim only when 1 KB beyond cap so each trim removes ~1 KB instead
// of O(n) on every append. Amortises copy cost to ~1 per KB of stderr.
const TRIM_TRIGGER_BYTES: usize = STDERR_CAP_BYTES + 1024;

pub struct StderrCappedBuffer {
    buf: String,
    trimmed: bool,
}

impl StderrCappedBuffer {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            trimmed: false,
        }
    }

    pub fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
        if self.buf.len() > TRIM_TRIGGER_BYTES {
            self.trim();
        }
    }

    fn trim(&mut self) {
        let drop_bytes = self.buf.len() - STDERR_CAP_BYTES;
        let mut split = drop_bytes;
        while split < self.buf.len() && !self.buf.is_char_boundary(split) {
            split += 1;
        }
        self.buf.drain(..split);
        self.trimmed = true;
    }

    pub fn snapshot(&self) -> String {
        self.buf.clone()
    }

    pub fn was_truncated(&self) -> bool {
        self.trimmed
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

impl Default for StderrCappedBuffer {
    fn default() -> Self {
        Self::new()
    }
}
