/// Content area: main agent output region (borderless).
mod line_cache;
mod message_lines;
mod tool_display;
mod welcome;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;

pub use line_cache::LineCache;
pub use message_lines::{message_to_lines, streaming_to_lines};

/// Render the content area — no border, no title, content fills the area.
///
/// Lines are pre-wrapped to terminal width via textwrap, so each Line
/// equals one visual row. `lines.len()` is the exact visual line count.
pub fn render_progress(
    f: &mut Frame,
    state: &SessionState,
    scroll_offset: u16,
    line_cache: &mut LineCache,
    area: Rect,
) {
    let visible_h = area.height as usize;
    if visible_h == 0 {
        return;
    }

    // Update cache with width for pre-wrapping (resize triggers full rebuild)
    line_cache.update(&state.messages, area.width);

    // Streaming lines (pre-wrapped at current width)
    let streaming = streaming_to_lines(&state.streaming_text, area.width);

    // Thinking indicator (shown during active thinking)
    let thinking_lines = if state.thinking_active {
        let token_est = state.streaming_thinking.len() as u32 / 4;
        let label = format!("Thinking... ({token_est} tokens)");
        vec![Line::from(Span::styled(
            label,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::DIM),
        ))]
    } else {
        vec![]
    };

    // Window: lines are already visual rows
    let window_size = visible_h + scroll_offset as usize;
    let cached_tail = line_cache.tail(window_size);

    // Build the render lines: cached tail + thinking + streaming
    let mut lines = Vec::with_capacity(cached_tail.len() + thinking_lines.len() + streaming.len());
    lines.extend_from_slice(cached_tail);
    lines.extend(thinking_lines);
    lines.extend(streaming);

    // Scroll: lines.len() == visual line count (pre-wrapped), so this is exact
    let window_lines = lines.len() as u16;
    let scroll_row = window_lines
        .saturating_sub(visible_h as u16)
        .saturating_sub(scroll_offset);

    let paragraph = Paragraph::new(lines).scroll((scroll_row, 0));
    f.render_widget(paragraph, area);
}
