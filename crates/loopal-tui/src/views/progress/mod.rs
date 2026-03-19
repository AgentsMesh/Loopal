/// Progress area: main chat/agent output region.
mod line_cache;
mod message_lines;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use loopal_session::state::SessionState;

pub use line_cache::LineCache;
pub use message_lines::{message_to_lines, streaming_to_lines};

/// Render the progress (chat) area.
///
/// Lines are pre-wrapped to terminal width via textwrap, so each Line
/// equals one visual row. No Paragraph::wrap() needed; `lines.len()`
/// is the exact visual line count, making scroll arithmetic correct.
pub fn render_progress(
    f: &mut Frame,
    state: &SessionState,
    scroll_offset: u16,
    line_cache: &mut LineCache,
    area: Rect,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Chat ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible_h = inner.height as usize;
    if visible_h == 0 {
        return;
    }

    // Update cache with width for pre-wrapping (resize triggers full rebuild)
    line_cache.update(&state.messages, inner.width);

    // Streaming lines (pre-wrapped at current width)
    let streaming = streaming_to_lines(&state.streaming_text, inner.width);

    // Window: lines are already visual rows, no 4x buffer needed
    let window_size = visible_h + scroll_offset as usize;
    let cached_tail = line_cache.tail(window_size);

    // Build the render lines: cached tail + streaming
    let mut lines = Vec::with_capacity(cached_tail.len() + streaming.len());
    lines.extend_from_slice(cached_tail);
    lines.extend(streaming);

    // Scroll: lines.len() == visual line count (pre-wrapped), so this is exact
    let window_lines = lines.len() as u16;
    let scroll_row = window_lines
        .saturating_sub(visible_h as u16)
        .saturating_sub(scroll_offset);

    let paragraph = Paragraph::new(lines).scroll((scroll_row, 0));
    f.render_widget(paragraph, inner);
}
