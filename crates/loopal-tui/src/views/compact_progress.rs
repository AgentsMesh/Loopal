use ratatui::prelude::*;

use super::inline_banner;

pub fn render_compact_banner(f: &mut Frame, message: &str, area: Rect) {
    inline_banner::render_inline_banner(
        f,
        "▼ ",
        message,
        Color::Cyan,
        Color::Rgb(15, 30, 50),
        area,
    );
}

pub fn banner_height(banner: &Option<String>) -> u16 {
    inline_banner::banner_height(banner)
}
