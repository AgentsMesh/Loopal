//! Tests for text_width — terminal column-width helpers.

use loopal_tui::views::text_width::{display_width, truncate_to_width};

#[test]
fn ascii_width_matches_char_count() {
    assert_eq!(display_width("hello"), 5);
    assert_eq!(display_width(""), 0);
}

#[test]
fn cjk_characters_are_two_columns() {
    assert_eq!(display_width("中文"), 4);
    assert_eq!(display_width("日本語"), 6);
}

#[test]
fn mixed_ascii_and_cjk_width() {
    assert_eq!(display_width("abc中文"), 7);
}

#[test]
fn truncate_empty_when_max_zero() {
    let (out, w) = truncate_to_width("hello", 0);
    assert_eq!(out, "");
    assert_eq!(w, 0);
}

#[test]
fn truncate_ascii_by_width() {
    let (out, w) = truncate_to_width("abcdef", 3);
    assert_eq!(out, "abc");
    assert_eq!(w, 3);
}

#[test]
fn truncate_respects_cjk_full_width() {
    // max_width=3 — "中" is 2 wide, "a" is 1 → "中a" fits (width 3).
    let (out, w) = truncate_to_width("中a文", 3);
    assert_eq!(out, "中a");
    assert_eq!(w, 3);
}

#[test]
fn truncate_stops_before_straddling_boundary() {
    // max_width=3 — "中" is 2 wide; adding "文" (2 wide) would make 4.
    let (out, w) = truncate_to_width("中文", 3);
    assert_eq!(out, "中");
    assert_eq!(w, 2);
}

#[test]
fn truncate_passes_short_input_through() {
    let (out, w) = truncate_to_width("hi", 100);
    assert_eq!(out, "hi");
    assert_eq!(w, 2);
}

#[test]
fn truncate_drops_ascii_control_chars() {
    let (out, w) = truncate_to_width("a\0b\x07c", 5);
    assert_eq!(out, "abc");
    assert_eq!(w, 3);
}

#[test]
fn truncate_drops_ansi_escape_bytes_as_controls() {
    // \x1b (ESC) is a control char; stripped so the rest renders cleanly.
    let (out, w) = truncate_to_width("a\x1b[31mred\x1b[0m", 10);
    assert!(!out.contains('\x1b'));
    assert!(w >= 4); // "a" + "[31mred" (bracketed text remains visible)
}
