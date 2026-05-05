/// Incremental line cache: avoids O(N) message-to-line conversion every frame.
///
/// Tracks how many messages have been converted and only processes new ones.
/// Supports windowed access to avoid full-clone of the entire history.
use ratatui::prelude::*;

use loopal_view_state::SessionMessage;

use super::message_lines::message_to_lines;

#[derive(Default)]
pub struct LineCache {
    /// Cached lines from fully-committed messages.
    lines: Vec<Line<'static>>,
    /// Number of SessionMessages that have been cached.
    cached_count: usize,
    /// Line index where the last cached message starts.
    last_msg_start: usize,
    /// Fingerprint of the last cached message (to detect in-place mutation).
    last_msg_fingerprint: u64,
    /// Terminal width used when building the cache (for resize detection).
    cached_width: u16,
}

impl LineCache {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            cached_count: 0,
            last_msg_start: 0,
            last_msg_fingerprint: 0,
            cached_width: 0,
        }
    }

    /// Update the cache and return total line count.
    pub fn update(&mut self, messages: &[SessionMessage], width: u16) -> usize {
        let msg_count = messages.len();

        // Width changed (terminal resize) — full rebuild
        if width != self.cached_width {
            self.reset();
            self.cached_width = width;
        }

        if msg_count < self.cached_count {
            self.reset();
            self.cached_width = width;
        }

        if self.cached_count > 0 && self.cached_count <= msg_count {
            let fp = fingerprint(&messages[self.cached_count - 1]);
            if fp != self.last_msg_fingerprint {
                self.lines.truncate(self.last_msg_start);
                self.cached_count -= 1;
            }
        }

        for msg in messages.iter().skip(self.cached_count) {
            self.last_msg_start = self.lines.len();
            self.lines.extend(message_to_lines(msg, width));
        }

        if let Some(last) = messages.last() {
            self.last_msg_fingerprint = fingerprint(last);
        }
        self.cached_count = msg_count;

        self.lines.len()
    }

    /// Return a slice of cached lines at an absolute position.
    pub fn slice(&self, start: usize, len: usize) -> &[Line<'static>] {
        let s = start.min(self.lines.len());
        let e = (s + len).min(self.lines.len());
        &self.lines[s..e]
    }

    /// Total number of cached lines.
    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }

    fn reset(&mut self) {
        self.lines.clear();
        self.cached_count = 0;
        self.last_msg_start = 0;
        self.last_msg_fingerprint = 0;
        self.cached_width = 0;
    }
}

/// Cheap fingerprint of a SessionMessage capturing all mutable parts.
fn fingerprint(msg: &SessionMessage) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    h = mix(h, msg.content.len() as u64);
    h = mix(h, msg.tool_calls.len() as u64);
    for tc in &msg.tool_calls {
        h = mix(h, tc.status as u64);
        h = mix(h, tc.summary.len() as u64);
        h = mix(h, tc.result.as_ref().map_or(0, |r| r.len()) as u64);
        h = mix(h, tc.duration_ms.unwrap_or(0));
        h = mix(h, tc.progress_tail.as_ref().map_or(0, |t| t.len()) as u64);
        // Running/pending tools: include sub-second elapsed for timer refresh.
        if tc.status.is_active() {
            h = mix(
                h,
                tc.started_at
                    .map_or(0, |t| t.elapsed().as_millis() as u64 / 200),
            );
        }
    }
    h
}

fn mix(h: u64, val: u64) -> u64 {
    (h ^ val).wrapping_mul(0x100000001b3)
}
