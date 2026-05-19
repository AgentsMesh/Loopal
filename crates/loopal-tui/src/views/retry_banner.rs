//! Transient retry error banner — in-place overlay between separator and input.
//!
//! Appears during LLM API retries, auto-clears on success.

use ratatui::prelude::*;

use super::inline_banner;

/// Render the retry error banner (1 row, yellow text on dark background).
///
/// ```text
/// ⟳ API error: status=502. Retrying in 4.0s (2/6)
/// ```
pub fn render_retry_banner(f: &mut Frame, message: &str, area: Rect) {
    inline_banner::render_inline_banner(
        f,
        "⟳ ",
        message,
        Color::Yellow,
        Color::Rgb(50, 35, 15),
        area,
    );
}

/// Height for the retry banner: 1 if present, 0 if absent.
pub fn banner_height(banner: &Option<String>) -> u16 {
    inline_banner::banner_height(banner)
}
