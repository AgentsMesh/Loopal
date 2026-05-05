//! String and JSON truncation utilities for the session layer.

use loopal_tool_api::truncate_output;

const RESULT_STORAGE_MAX_LINES: usize = 200;
const RESULT_STORAGE_MAX_BYTES: usize = 10_000;

/// Loose storage-protection truncation for tool results.
/// Preserves up to 200 lines / 10 KB — enough for session replay, not display.
pub(crate) fn truncate_result_for_storage(result: &str) -> String {
    truncate_output(result, RESULT_STORAGE_MAX_LINES, RESULT_STORAGE_MAX_BYTES)
}

/// Truncate a JSON value to a compact string, capping at `max_len` chars.
pub(crate) fn truncate_json(value: &serde_json::Value, max_len: usize) -> String {
    let s = value.to_string();
    truncate_str(&s, max_len)
}

/// Truncate a string to `max_len` chars (respecting char boundaries), appending "...".
pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
