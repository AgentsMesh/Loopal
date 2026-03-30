//! Shared text utilities for TUI rendering.

/// Truncate a string to fit within `max_width` display columns (CJK-safe).
pub(crate) fn truncate_to_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut buf = String::new();
    let mut col = 0;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + cw > max_width {
            break;
        }
        buf.push(ch);
        col += cw;
    }
    buf
}
