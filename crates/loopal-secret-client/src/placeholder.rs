use once_cell::sync::Lazy;
use regex::Regex;

pub static AUTHOR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\{secret:([a-z][a-z0-9_]*)\}\}").expect("author regex"));

pub static WIRE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<secret_ref:([a-z][a-z0-9_]*)>").expect("wire regex"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_re_captures_simple_name() {
        let caps: Vec<_> = AUTHOR_RE
            .captures_iter("a {{secret:key1}} b")
            .map(|c| c[1].to_string())
            .collect();
        assert_eq!(caps, vec!["key1".to_string()]);
    }

    #[test]
    fn wire_re_captures_simple_name() {
        let caps: Vec<_> = WIRE_RE
            .captures_iter("<secret_ref:tok>")
            .map(|c| c[1].to_string())
            .collect();
        assert_eq!(caps, vec!["tok".to_string()]);
    }

    #[test]
    fn name_must_start_with_lowercase_letter() {
        assert!(AUTHOR_RE.captures("{{secret:1abc}}").is_none());
        assert!(AUTHOR_RE.captures("{{secret:_abc}}").is_none());
        assert!(WIRE_RE.captures("<secret_ref:1abc>").is_none());
    }

    #[test]
    fn name_allows_digits_and_underscores_after_first_letter() {
        assert!(AUTHOR_RE.captures("{{secret:api_key_2}}").is_some());
        assert!(WIRE_RE.captures("<secret_ref:api_key_2>").is_some());
    }
}
