//! Multiline cursor navigation helpers.
//!
//! Converts between byte-offset cursors and (row, col) positions,
//! accounting for both hard newlines (`\n`) and soft wrapping.

use unicode_width::UnicodeWidthChar;

/// A visual line produced by splitting on `\n` then soft-wrapping.
#[derive(Debug)]
pub struct VisualLine {
    /// Byte offset of this visual line's start within the full string.
    pub byte_start: usize,
    /// Byte length of this visual line (excluding any trailing `\n`).
    pub byte_len: usize,
}

/// Build the list of visual lines for `text`, wrapping at `wrap_width`
/// display columns. A `wrap_width` of 0 disables wrapping.
pub fn visual_lines(text: &str, wrap_width: usize) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let wrap = if wrap_width == 0 {
        usize::MAX
    } else {
        wrap_width
    };
    for (hard_start, hard_line) in split_newlines(text) {
        if hard_line.is_empty() {
            lines.push(VisualLine {
                byte_start: hard_start,
                byte_len: 0,
            });
            continue;
        }
        let mut col = 0usize;
        let mut line_byte_start = hard_start;
        for (i, ch) in hard_line.char_indices() {
            let w = ch.width().unwrap_or(0);
            if col + w > wrap && col > 0 {
                lines.push(VisualLine {
                    byte_start: line_byte_start,
                    byte_len: hard_start + i - line_byte_start,
                });
                line_byte_start = hard_start + i;
                col = w;
            } else {
                col += w;
            }
        }
        lines.push(VisualLine {
            byte_start: line_byte_start,
            byte_len: hard_start + hard_line.len() - line_byte_start,
        });
    }
    if lines.is_empty() {
        lines.push(VisualLine {
            byte_start: 0,
            byte_len: 0,
        });
    }
    lines
}

/// Find the visual (row, display_col) for a byte cursor position.
pub fn cursor_to_row_col(text: &str, cursor: usize, lines: &[VisualLine]) -> (usize, usize) {
    for (row, vl) in lines.iter().enumerate() {
        let line_end = vl.byte_start + vl.byte_len;
        if cursor >= vl.byte_start && (cursor < line_end || row == lines.len() - 1) {
            let slice = &text[vl.byte_start..cursor.min(line_end)];
            let col: usize = slice.chars().map(|c| c.width().unwrap_or(0)).sum();
            return (row, col);
        }
    }
    (0, 0)
}

/// Move cursor up one visual row. Returns `None` if already on the first row.
pub fn cursor_up(text: &str, cursor: usize, wrap_width: usize) -> Option<usize> {
    let lines = visual_lines(text, wrap_width);
    let (row, col) = cursor_to_row_col(text, cursor, &lines);
    if row == 0 {
        return None;
    }
    Some(col_to_byte(&lines[row - 1], text, col))
}

/// Move cursor down one visual row. Returns `None` if already on the last row.
pub fn cursor_down(text: &str, cursor: usize, wrap_width: usize) -> Option<usize> {
    let lines = visual_lines(text, wrap_width);
    let (row, col) = cursor_to_row_col(text, cursor, &lines);
    if row + 1 >= lines.len() {
        return None;
    }
    Some(col_to_byte(&lines[row + 1], text, col))
}

/// Move cursor to the start of the current visual line.
pub fn line_home(text: &str, cursor: usize, wrap_width: usize) -> usize {
    let lines = visual_lines(text, wrap_width);
    let (row, _) = cursor_to_row_col(text, cursor, &lines);
    lines[row].byte_start
}

/// Move cursor to the end of the current visual line.
pub fn line_end(text: &str, cursor: usize, wrap_width: usize) -> usize {
    let lines = visual_lines(text, wrap_width);
    let (row, _) = cursor_to_row_col(text, cursor, &lines);
    lines[row].byte_start + lines[row].byte_len
}

/// Check whether the input contains multiple visual lines.
pub fn is_multiline(text: &str, wrap_width: usize) -> bool {
    visual_lines(text, wrap_width).len() > 1
}

/// Split text on `\n`, yielding (byte_offset, slice) pairs.
fn split_newlines(text: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            result.push((start, &text[start..i]));
            start = i + 1;
        }
    }
    result.push((start, &text[start..]));
    result
}

/// Find the byte offset for a target display column in a visual line.
fn col_to_byte(vl: &VisualLine, text: &str, target_col: usize) -> usize {
    let slice = &text[vl.byte_start..vl.byte_start + vl.byte_len];
    let mut col = 0usize;
    for (i, ch) in slice.char_indices() {
        let w = ch.width().unwrap_or(0);
        if col + w > target_col {
            return vl.byte_start + i;
        }
        col += w;
    }
    vl.byte_start + vl.byte_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_no_wrap() {
        let lines = visual_lines("hello", 80);
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].byte_start, lines[0].byte_len), (0, 5));
    }

    #[test]
    fn hard_newlines() {
        let text = "ab\ncd\nef";
        let lines = visual_lines(text, 80);
        assert_eq!(lines.len(), 3);
        assert_eq!(cursor_to_row_col(text, 0, &lines), (0, 0));
        assert_eq!(cursor_to_row_col(text, 3, &lines), (1, 0));
        assert_eq!(cursor_to_row_col(text, 6, &lines), (2, 0));
    }

    #[test]
    fn soft_wrap() {
        let lines = visual_lines("abcdef", 3);
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].byte_len, lines[1].byte_start), (3, 3));
    }

    #[test]
    fn cursor_navigation() {
        let text = "ab\ncd";
        assert_eq!(cursor_up(text, 3, 80), Some(0));
        assert_eq!(cursor_down(text, 0, 80), Some(3));
        assert_eq!(cursor_up(text, 0, 80), None);
        assert_eq!(cursor_down(text, 3, 80), None);
    }

    #[test]
    fn cjk_wrap() {
        assert_eq!(visual_lines("你好世界", 4).len(), 2);
    }

    #[test]
    fn empty_input() {
        let lines = visual_lines("", 80);
        assert_eq!(lines.len(), 1);
        assert_eq!(cursor_to_row_col("", 0, &lines), (0, 0));
    }
}
