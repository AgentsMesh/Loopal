use loopal_tool_api::truncate_output;

const RESULT_STORAGE_MAX_LINES: usize = 200;
const RESULT_STORAGE_MAX_BYTES: usize = 10_000;

pub(crate) fn truncate_result_for_storage(result: &str) -> String {
    truncate_output(result, RESULT_STORAGE_MAX_LINES, RESULT_STORAGE_MAX_BYTES)
}

pub(crate) fn truncate_json(value: &serde_json::Value, max_len: usize) -> String {
    let s = value.to_string();
    truncate_str(&s, max_len)
}

fn truncate_str(s: &str, max_len: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truncate_str_returns_inputs_under_limit() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("", 5), "");
    }

    #[test]
    fn truncate_str_appends_ellipsis_over_limit() {
        let out = truncate_str("0123456789abcdef", 8);
        assert_eq!(out, "01234567...");
    }

    #[test]
    fn truncate_str_respects_char_boundary() {
        let s = "中文测试abc";
        let out = truncate_str(s, 4);
        assert!(out.is_char_boundary(out.len() - 3));
        assert!(out.ends_with("..."));
    }

    #[test]
    fn truncate_json_compact_representation() {
        let v = json!({"a": 1, "b": 2});
        let out = truncate_json(&v, 100);
        assert!(out.contains("\"a\":1"));
        assert!(out.contains("\"b\":2"));
    }

    #[test]
    fn truncate_json_truncates_long_value() {
        let v = json!({"data": "x".repeat(200)});
        let out = truncate_json(&v, 50);
        assert!(out.len() <= 53);
        assert!(out.ends_with("..."));
    }
}
