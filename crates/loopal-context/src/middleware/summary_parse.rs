/// Return the body of the first `<summary>...</summary>` block, trimmed.
/// Falls back to the full response (trimmed) when no tag is present.
pub fn extract_summary(raw: &str) -> &str {
    let open = "<summary>";
    let close = "</summary>";
    if let Some(start) = raw.find(open) {
        let body_start = start + open.len();
        if let Some(end_rel) = raw[body_start..].find(close) {
            return raw[body_start..body_start + end_rel].trim();
        }
    }
    raw.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_summary_block() {
        let raw = "<analysis>drafting</analysis>\n<summary>final body</summary>";
        assert_eq!(extract_summary(raw), "final body");
    }

    #[test]
    fn handles_whitespace_around_summary() {
        let raw = "<summary>\n  multi\n  line  \n</summary>";
        assert_eq!(extract_summary(raw), "multi\n  line");
    }

    #[test]
    fn fallback_when_no_tag() {
        let raw = "  bare summary text  ";
        assert_eq!(extract_summary(raw), "bare summary text");
    }

    #[test]
    fn fallback_when_only_open_tag() {
        let raw = "<summary>not closed";
        assert_eq!(extract_summary(raw), "<summary>not closed");
    }

    #[test]
    fn picks_first_summary_block() {
        let raw = "<summary>one</summary><summary>two</summary>";
        assert_eq!(extract_summary(raw), "one");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(extract_summary(""), "");
    }

    #[test]
    fn analysis_only_falls_back_to_full_text() {
        let raw = "<analysis>draft only, no summary tag</analysis>";
        assert_eq!(extract_summary(raw), raw);
    }
}
