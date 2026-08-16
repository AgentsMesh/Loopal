use loopal_output_guard::{OutputGuard, OutputGuardError};
use secrecy::SecretString;

fn guard(seed: &[(&str, &str)]) -> OutputGuard {
    OutputGuard::new(
        &seed
            .iter()
            .map(|(name, value)| {
                (
                    (*name).to_string(),
                    SecretString::from((*value).to_string()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn redactor_build_error_is_content_free() {
    let error = loopal_output_guard::OutputGuardBuildError;
    assert_eq!(format!("{error}"), "secret redactor could not be built");
    assert_eq!(format!("{error:?}"), "OutputGuardBuildError");
}

#[test]
fn empty_seed_and_empty_plaintext_are_ignored() {
    let empty = guard(&[]);
    assert!(empty.is_empty());
    let seeded = guard(&[("ignored", "")]);
    assert!(seeded.is_empty());
    let output = seeded.guard_text("plain", 5).unwrap();
    let debug = format!("{output:?}");
    assert!(debug.contains("secret_count"));
    assert!(!debug.contains("plain"));
    assert_eq!(output.into_inner().as_str(), "plain");
}

#[test]
fn unbounded_redaction_reports_hits_and_owned_text() {
    let redacted = guard(&[("key", "secret")]).redact_text("a secret");
    assert_eq!(redacted.secret_names(), &["key"]);
    let debug = format!("{redacted:?}");
    assert!(!debug.contains("a secret"));
    assert!(!debug.contains("key"));
    assert_eq!(redacted.into_inner(), "a <secret_ref:key>");
}

#[test]
fn short_and_repeated_secrets_are_redacted() {
    let guarded = guard(&[("pin", "12")]).guard_text("12 and 12", 64).unwrap();
    assert_eq!(guarded.secret_names(), &["pin"]);
    assert_eq!(
        guarded.into_inner().into_string(),
        "<secret_ref:pin> and <secret_ref:pin>"
    );
}

#[test]
fn duplicate_plaintext_uses_first_seed_name() {
    let guarded = guard(&[("first", "same"), ("second", "same")])
        .guard_text("same", 32)
        .unwrap();
    assert_eq!(guarded.secret_names(), &["first"]);
    assert_eq!(guarded.into_inner().as_str(), "<secret_ref:first>");
}

#[test]
fn leftmost_longest_overlap_matches_runtime_semantics() {
    let guarded = guard(&[("short", "abcd"), ("long", "abcdef")])
        .guard_text("xxabcdefabcd", 64)
        .unwrap();
    assert_eq!(guarded.secret_names(), &["long", "short"]);
    assert_eq!(
        guarded.into_inner().as_str(),
        "xx<secret_ref:long><secret_ref:short>"
    );
}

#[test]
fn byte_limit_accepts_exact_unicode_boundary() {
    let output = guard(&[]).guard_text("é猫", 5).unwrap();
    assert_eq!(output.into_inner().as_str(), "é猫");
}

#[test]
fn byte_limit_rejects_without_splitting_unicode() {
    let error = guard(&[]).guard_text("é猫", 4).unwrap_err();
    assert_eq!(
        error,
        OutputGuardError::ByteLimitExceeded {
            actual_bytes: 5,
            max_bytes: 4,
        }
    );
}

#[test]
fn redaction_happens_before_the_byte_bound() {
    let guarded = guard(&[("key", "a-very-long-plaintext-secret")])
        .guard_text("a-very-long-plaintext-secret", 16)
        .unwrap();
    assert_eq!(guarded.into_inner().as_str(), "<secret_ref:key>");
}

#[test]
fn guarded_text_debug_does_not_include_content() {
    let guarded = guard(&[]).guard_text("private output", 64).unwrap();
    assert!(!format!("{:?}", guarded.into_inner()).contains("private output"));
}

#[test]
fn text_errors_never_contain_plaintext_secrets() {
    let secret = "private-value-1847";
    let error = guard(&[("key", secret)]).guard_text(secret, 1).unwrap_err();
    assert!(!format!("{error}").contains(secret));
    assert!(!format!("{error:?}").contains(secret));
}
