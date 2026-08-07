/// Incremental line cache: avoids O(N) message-to-line conversion every frame.
///
/// Tracks how many messages have been converted and only processes new ones.
/// Supports windowed access to avoid full-clone of the entire history.
use ratatui::prelude::*;

use loopal_view_state::SessionMessage;

use super::message_lines::message_to_lines_with_animation_elapsed;

#[derive(Default)]
pub struct LineCache {
    /// Cached lines from fully-committed messages.
    lines: Vec<Line<'static>>,
    /// Number of SessionMessages that have been cached.
    cached_count: usize,
    /// Line start and content fingerprint for every cached message.
    ///
    /// View-state can mutate an earlier assistant row after newer inbox/system
    /// rows have been appended (ToolProgress/ToolResult target tool-use id, not
    /// the last row). Per-message boundaries let us invalidate exactly the
    /// affected suffix instead of rebuilding the full conversation.
    message_starts: Vec<usize>,
    message_fingerprints: Vec<u64>,
    /// Cached message indices that still contain an active tool. These rows
    /// remain mutable even after a later message is appended, and their
    /// animation fingerprint advances on every spinner frame.
    active_message_indices: Vec<usize>,
    /// Terminal width used when building the cache (for resize detection).
    cached_width: u16,
}

impl LineCache {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            cached_count: 0,
            message_starts: Vec::new(),
            message_fingerprints: Vec::new(),
            active_message_indices: Vec::new(),
            cached_width: 0,
        }
    }

    /// Update the cache and return total line count.
    pub fn update(&mut self, messages: &[SessionMessage], width: u16) -> usize {
        self.update_with_animation_elapsed(messages, width, crate::animation::elapsed())
    }

    /// Update using the animation clock sampled by the current frame.
    pub(crate) fn update_with_animation_elapsed(
        &mut self,
        messages: &[SessionMessage],
        width: u16,
        animation_elapsed: std::time::Duration,
    ) -> usize {
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

        if self.cached_count > 0
            && self.cached_count <= msg_count
            && let Some(changed_at) = self.first_changed_index(messages, animation_elapsed)
        {
            self.invalidate_from(changed_at);
        }

        for (idx, msg) in messages.iter().enumerate().skip(self.cached_count) {
            self.message_starts.push(self.lines.len());
            self.lines.extend(message_to_lines_with_animation_elapsed(
                msg,
                width,
                animation_elapsed,
            ));
            self.message_fingerprints
                .push(fingerprint(msg, animation_elapsed));
            if has_active_tool(msg) {
                self.active_message_indices.push(idx);
            }
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
        self.message_starts.clear();
        self.message_fingerprints.clear();
        self.active_message_indices.clear();
        self.cached_width = 0;
    }

    /// Check every row that is allowed to mutate in place: all active-tool
    /// rows plus the most recently cached row (stream flushes can extend it).
    fn first_changed_index(
        &self,
        messages: &[SessionMessage],
        animation_elapsed: std::time::Duration,
    ) -> Option<usize> {
        self.active_message_indices
            .iter()
            .copied()
            .chain(std::iter::once(self.cached_count - 1))
            .filter(|idx| *idx < self.cached_count)
            .filter(|idx| {
                fingerprint(&messages[*idx], animation_elapsed) != self.message_fingerprints[*idx]
            })
            .min()
    }

    fn invalidate_from(&mut self, message_index: usize) {
        self.lines.truncate(self.message_starts[message_index]);
        self.message_starts.truncate(message_index);
        self.message_fingerprints.truncate(message_index);
        self.active_message_indices
            .retain(|idx| *idx < message_index);
        self.cached_count = message_index;
    }
}

fn has_active_tool(msg: &SessionMessage) -> bool {
    msg.tool_calls.iter().any(|tool| tool.state.is_active())
}

fn fingerprint(msg: &SessionMessage, animation_elapsed: std::time::Duration) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    h = mix(h, hash_str(&msg.content));
    h = mix(h, msg.tool_calls.len() as u64);
    for tc in &msg.tool_calls {
        h = mix(h, fingerprint_variant(&tc.state));
        h = mix(h, hash_str(&tc.summary));
        h = mix(h, tc.state.progress_tail().map_or(0, hash_str));
        if let Some(o) = tc.state.outcome() {
            h = mix(h, hash_str(o.content()));
        }
        if let Some(d) = tc.state.duration() {
            h = mix(h, d.as_millis() as u64);
        }
        if tc.state.is_active() {
            h = mix(h, animation_elapsed.as_millis() as u64 / 100);
        }
    }
    h
}

fn fingerprint_variant(state: &loopal_view_state::InvocationState) -> u64 {
    use loopal_view_state::InvocationState::*;
    match state {
        Pending => 1,
        Running { .. } => 2,
        Done { .. } => 3,
        Stale { .. } => 4,
        Cancelled { .. } => 5,
    }
}

fn hash_str(s: &str) -> u64 {
    // FNV-1a — fast content hash, no allocations.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= s.len() as u64;
    h
}

fn mix(h: u64, val: u64) -> u64 {
    (h ^ val).wrapping_mul(0x100000001b3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_protocol::AgentEventPayload;
    use loopal_view_state::ViewStateReducer;

    fn cached_text(cache: &LineCache) -> String {
        cache
            .slice(0, cache.total_lines())
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn earlier_active_tool_row_invalidates_after_a_later_system_row() {
        let mut reducer = ViewStateReducer::new("main");
        reducer.apply(AgentEventPayload::ToolCall {
            id: "tool-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/input"}),
        });
        reducer.apply(AgentEventPayload::ProviderWarning {
            message: "later system row".into(),
        });

        let mut cache = LineCache::new();
        cache.update_with_animation_elapsed(
            &reducer.state().agent.conversation.messages,
            100,
            std::time::Duration::ZERO,
        );
        let first = cached_text(&cache);
        assert!(first.contains("⠋ Read"), "initial spinner missing: {first}");
        assert!(first.contains("later system row"));

        cache.update_with_animation_elapsed(
            &reducer.state().agent.conversation.messages,
            100,
            std::time::Duration::from_millis(100),
        );
        let second = cached_text(&cache);
        assert!(
            second.contains("⠙ Read"),
            "earlier-row spinner did not advance: {second}"
        );

        reducer.apply(AgentEventPayload::ToolResult {
            id: "tool-1".into(),
            name: "Read".into(),
            result: "one\ntwo".into(),
            is_error: false,
            duration_ms: Some(10),
            metadata: None,
        });
        cache.update_with_animation_elapsed(
            &reducer.state().agent.conversation.messages,
            100,
            std::time::Duration::from_millis(100),
        );
        let completed = cached_text(&cache);
        assert!(
            completed.contains("● Read"),
            "terminal state did not invalidate earlier row: {completed}"
        );
        assert!(completed.contains("Read 2 lines"));
        assert!(completed.contains("later system row"));
    }
}
