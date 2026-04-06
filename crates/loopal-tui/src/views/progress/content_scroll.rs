/// Content area scroll state: encapsulates scroll offset, growth compensation,
/// and the incremental line cache.
///
/// Scroll model: `offset` = lines from the bottom of all content.
/// - `offset == 0` → auto-follow (viewport stays at bottom)
/// - `offset > 0` → viewport is pinned N lines above the bottom
///
/// When content grows while pinned, the offset is auto-compensated so the
/// viewport stays anchored at the same absolute position. Rendering uses
/// absolute position indexing (not `tail()`) to avoid the feedback loop
/// where window_size depends on offset and vice versa.
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;

use super::line_cache::LineCache;
use super::message_lines::streaming_to_lines;
use super::thinking_render;

/// Scroll + render state for the main content area.
pub struct ContentScroll {
    /// Lines from the bottom of all content (0 = auto-follow).
    pub offset: u16,
    /// Total content lines at previous render (for growth compensation).
    prev_total: usize,
    /// Incremental line cache for committed messages.
    line_cache: LineCache,
}

impl Default for ContentScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentScroll {
    pub fn new() -> Self {
        Self {
            offset: 0,
            prev_total: 0,
            line_cache: LineCache::new(),
        }
    }

    /// Reset to initial state (view switch, new conversation).
    pub fn reset(&mut self) {
        self.offset = 0;
        self.prev_total = 0;
        self.line_cache = LineCache::new();
    }

    /// Scroll to bottom (auto-follow mode).
    pub fn to_bottom(&mut self) {
        self.offset = 0;
    }

    /// Scroll up by N lines.
    pub fn scroll_up(&mut self, lines: u16) {
        self.offset = self.offset.saturating_add(lines);
    }

    /// Scroll down by N lines.
    pub fn scroll_down(&mut self, lines: u16) {
        self.offset = self.offset.saturating_sub(lines);
    }

    /// Render the content area into the given frame region.
    pub fn render(&mut self, f: &mut Frame, state: &SessionState, area: Rect) {
        let visible_h = area.height as usize;
        if visible_h == 0 {
            return;
        }

        let conv = state.active_conversation();

        self.line_cache.update(&conv.messages, area.width);

        let streaming = streaming_to_lines(&conv.streaming_text, area.width);
        let thinking_lines = if conv.thinking_active {
            thinking_render::streaming_thinking_lines(&conv.streaming_thinking, area.width)
        } else {
            vec![]
        };

        // Total logical content = cache + thinking + streaming
        let cache_total = self.line_cache.total_lines();
        let total = cache_total + thinking_lines.len() + streaming.len();

        // Compensate offset for content growth while pinned.
        // Skip first frame (prev_total == 0) to avoid initial blowup.
        if self.offset > 0 && self.prev_total > 0 && total > self.prev_total {
            let delta = (total - self.prev_total).min(u16::MAX as usize) as u16;
            self.offset = self.offset.saturating_add(delta);
        }
        self.prev_total = total;

        // Absolute top line of the viewport (clamped)
        let max_scroll = total.saturating_sub(visible_h);
        let clamped = (self.offset as usize).min(max_scroll);
        let top_line = max_scroll.saturating_sub(clamped);

        // Slice cache portion visible in the viewport
        let cache_start = top_line.min(cache_total);
        let cache_end = (top_line + visible_h).min(cache_total);
        let visible_cache = self.line_cache.slice(cache_start, cache_end - cache_start);

        let mut lines = Vec::with_capacity(visible_h);
        lines.extend_from_slice(visible_cache);

        // Append ephemeral (thinking + streaming) lines if viewport extends past cache
        if top_line + visible_h > cache_total {
            let eph_start = top_line.saturating_sub(cache_total);
            let all_ephemeral: Vec<Line<'_>> =
                thinking_lines.into_iter().chain(streaming).collect();
            let eph_end = (top_line + visible_h - cache_total).min(all_ephemeral.len());
            if eph_start < eph_end {
                lines.extend_from_slice(&all_ephemeral[eph_start..eph_end]);
            }
        }

        let paragraph = Paragraph::new(lines).scroll((0, 0));
        f.render_widget(paragraph, area);
    }
}
