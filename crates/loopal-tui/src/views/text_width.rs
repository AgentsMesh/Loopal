//! Terminal column-width helpers for panel rendering.
//!
//! Panel lines reserve a fixed number of *terminal columns* for a label.
//! `str::len()` counts bytes and `str::chars().count()` counts Unicode
//! scalars — neither matches a terminal cell: CJK, full-width, and some
//! emoji render as two cells while ASCII renders as one.
//!
//! All panels (tasks, bg_tasks, crons) should route width math through
//! these helpers so labels stay aligned regardless of script.
//!
//! ## Caveats
//!
//! - ANSI escape sequences (`\x1b[…m`) are **not** stripped here. Callers
//!   must feed raw text only. Panels pass plain user strings.
//! - Zero-width combining marks and emoji ZWJ sequences are reported by
//!   `unicode-width` per-codepoint, so a grapheme cluster like `👨‍👩‍👧`
//!   or `é` may be *under*-measured. We accept this approximation — real
//!   panels bound line width by max_width with a safety margin.
//! - ASCII control characters (`\t`, `\n`, ANSI `\x1b`, bell `\x07`, …)
//!   are **filtered out** during truncation to prevent terminal corruption.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Display width of `s` in terminal cells. ANSI escapes, if present,
/// are treated as ordinary characters — callers must pre-strip them.
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate `s` so its terminal width does not exceed `max_width`.
/// ASCII control characters are silently dropped to avoid emitting
/// escape-like bytes into the terminal.
///
/// Returns the truncated string along with its actual display width
/// (which may be `max_width - 1` when a 2-wide glyph would straddle the
/// boundary).
pub fn truncate_to_width(s: &str, max_width: usize) -> (String, usize) {
    if max_width == 0 {
        return (String::new(), 0);
    }
    let mut out = String::new();
    let mut width = 0usize;
    for ch in s.chars() {
        if ch.is_control() {
            continue;
        }
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > max_width {
            break;
        }
        width += w;
        out.push(ch);
    }
    (out, width)
}
