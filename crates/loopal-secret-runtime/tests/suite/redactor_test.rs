use loopal_secret_runtime::Redactor;
use secrecy::SecretString;

fn pairs(items: &[(&str, &str)]) -> Vec<(String, SecretString)> {
    items
        .iter()
        .map(|(n, v)| ((*n).to_string(), SecretString::from((*v).to_string())))
        .collect()
}

#[test]
fn empty_redactor_is_no_op() {
    let r = Redactor::from_pairs(&[]);
    assert!(r.is_empty());
    let (out, hits) = r.scan_and_redact("plain text with no secret");
    assert_eq!(out, "plain text with no secret");
    assert!(hits.is_empty());
}

#[test]
fn short_value_is_still_redacted() {
    // Consistency with vault: any non-empty plaintext gets redacted
    // regardless of length. Caller takes responsibility for false-positives
    // when storing very short secrets.
    let r = Redactor::from_pairs(&pairs(&[("short", "abc")]));
    assert!(!r.is_empty());
    let (out, hits) = r.scan_and_redact("xx abc yy");
    assert_eq!(out, "xx <secret_ref:short> yy");
    assert_eq!(hits, vec!["short".to_string()]);
}

#[test]
fn empty_value_not_added_to_matcher() {
    // Empty string would otherwise match every position; explicitly skipped.
    let r = Redactor::from_pairs(&pairs(&[("k", "")]));
    assert!(r.is_empty());
}

#[test]
fn long_value_redacted_to_placeholder() {
    let r = Redactor::from_pairs(&pairs(&[("openai", "sk-abc12345")]));
    let (out, hits) = r.scan_and_redact("response: sk-abc12345 done");
    assert_eq!(out, "response: <secret_ref:openai> done");
    assert_eq!(hits, vec!["openai".to_string()]);
}

#[test]
fn output_with_no_match_returned_unchanged() {
    let r = Redactor::from_pairs(&pairs(&[("openai", "sk-abc12345")]));
    let (out, hits) = r.scan_and_redact("clean text");
    assert_eq!(out, "clean text");
    assert!(hits.is_empty());
}

#[test]
fn longest_pattern_wins_when_substring_overlaps() {
    let r = Redactor::from_pairs(&pairs(&[
        ("short_one", "abcdefgh"),
        ("long_one", "abcdefghijkl"),
    ]));
    let (out, hits) = r.scan_and_redact("xx abcdefghijkl yy");
    assert_eq!(out, "xx <secret_ref:long_one> yy");
    assert_eq!(hits, vec!["long_one".to_string()]);
}

#[test]
fn multiple_distinct_secrets_in_one_output() {
    let r = Redactor::from_pairs(&pairs(&[
        ("openai", "sk-abc12345"),
        ("hf", "hf-tokenvalue"),
    ]));
    let (out, mut hits) = r.scan_and_redact("a=sk-abc12345 b=hf-tokenvalue");
    assert_eq!(out, "a=<secret_ref:openai> b=<secret_ref:hf>");
    hits.sort();
    assert_eq!(hits, vec!["hf".to_string(), "openai".to_string()]);
}

#[test]
fn same_secret_redacted_multiple_times_dedup_in_hits() {
    let r = Redactor::from_pairs(&pairs(&[("k", "abcdefgh")]));
    let (out, hits) = r.scan_and_redact("v1=abcdefgh and v2=abcdefgh");
    assert_eq!(out, "v1=<secret_ref:k> and v2=<secret_ref:k>");
    assert_eq!(hits, vec!["k".to_string()]);
}

#[test]
fn placeholder_string_itself_not_a_false_positive() {
    let r = Redactor::from_pairs(&pairs(&[("k", "12345678")]));
    let (out, hits) = r.scan_and_redact("<missing-secret:k>");
    assert_eq!(out, "<missing-secret:k>");
    assert!(hits.is_empty());
}

#[test]
fn arbitrary_length_value_is_redacted() {
    // Length is no longer gated — was previously `boundary_min_length_value_redacted`.
    let r = Redactor::from_pairs(&pairs(&[("k", "12345678")]));
    assert!(!r.is_empty());
    let (out, _) = r.scan_and_redact("v=12345678");
    assert_eq!(out, "v=<secret_ref:k>");
}

#[test]
fn very_short_value_redacted_and_warns_user_via_false_positive_risk() {
    // 4-char secret: gets redacted as designed. The caller chose to store
    // "1234" in the vault; any string containing "1234" is now flagged.
    let r = Redactor::from_pairs(&pairs(&[("pin", "1234")]));
    let (out, _) = r.scan_and_redact("port 1234 listening");
    assert_eq!(out, "port <secret_ref:pin> listening");
}
