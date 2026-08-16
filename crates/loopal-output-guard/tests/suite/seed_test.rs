use loopal_output_guard::{
    FinalSinkRedactionSeed, MAX_STREAM_SECRET_BYTES, MAX_STREAM_SECRET_NAME_BYTES,
    MAX_STREAM_SECRET_PATTERNS, MAX_STREAM_SECRET_TOTAL_BYTES,
};
use secrecy::ExposeSecret;

#[test]
fn deduplicates_plaintext_and_retains_rotated_values() {
    let seed = FinalSinkRedactionSeed::new();
    seed.observe("token", "old-value".into()).unwrap();
    seed.observe("alias", "old-value".into()).unwrap();
    seed.observe("token", "new-value".into()).unwrap();

    let snapshot = seed.snapshot().unwrap();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].0, "token");
    assert_eq!(snapshot[0].1.expose_secret(), "old-value");
    assert_eq!(snapshot[1].1.expose_secret(), "new-value");
}

#[test]
fn empty_values_are_ignored_and_debug_reports_only_the_count() {
    let seed = FinalSinkRedactionSeed::new();
    seed.observe("empty", "".into()).unwrap();
    seed.observe("token", "secret-value".into()).unwrap();

    assert_eq!(seed.snapshot().unwrap().len(), 1);
    let debug = format!("{seed:?}");
    assert!(debug.contains("FinalSinkRedactionSeed"));
    assert!(debug.contains("entry_count"));
    assert!(!debug.contains("secret-value"));
}

#[test]
fn rejects_pattern_capacity_without_dropping_existing_values() {
    let seed = FinalSinkRedactionSeed::new();
    for index in 0..MAX_STREAM_SECRET_PATTERNS {
        seed.observe(format!("secret_{index}"), format!("value-{index}").into())
            .unwrap();
    }

    assert!(seed.observe("overflow", "one-more".into()).is_err());
    assert_eq!(seed.snapshot().unwrap().len(), MAX_STREAM_SECRET_PATTERNS);
}

#[test]
fn rejects_invalid_name_value_and_total_byte_limits() {
    let seed = FinalSinkRedactionSeed::new();
    assert!(
        seed.observe("n".repeat(MAX_STREAM_SECRET_NAME_BYTES + 1), "value".into())
            .is_err()
    );
    assert!(
        seed.observe("large", "x".repeat(MAX_STREAM_SECRET_BYTES + 1).into())
            .is_err()
    );

    for index in 0..MAX_STREAM_SECRET_TOTAL_BYTES / MAX_STREAM_SECRET_BYTES {
        let value = format!("{index}{}", "x".repeat(MAX_STREAM_SECRET_BYTES - 1));
        seed.observe(format!("chunk_{index}"), value.into())
            .unwrap();
    }
    assert!(seed.observe("total_overflow", "different".into()).is_err());
}
