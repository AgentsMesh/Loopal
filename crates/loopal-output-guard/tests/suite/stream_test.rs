use loopal_output_guard::{
    MAX_STREAM_SECRET_BYTES, MAX_STREAM_SECRET_NAME_BYTES, MAX_STREAM_SECRET_PATTERNS,
    MAX_STREAM_SECRET_TOTAL_BYTES, StreamingOutputGuard, StreamingOutputGuardFinished,
};
use secrecy::SecretString;

fn guard(seed: &[(&str, &str)]) -> StreamingOutputGuard {
    StreamingOutputGuard::new(
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
fn redacts_a_secret_split_at_every_byte_boundary() {
    for split in 0..="before-secret-after".len() {
        let mut guard = guard(&[("key", "secret")]);
        let input = b"before-secret-after";
        let mut output = guard.push(&input[..split]).unwrap().into_inner();
        output.extend(guard.push(&input[split..]).unwrap().into_inner());
        output.extend(guard.finish().into_inner());
        assert_eq!(output, b"before-<secret_ref:key>-after");
    }
}

#[test]
fn one_byte_chunks_preserve_leftmost_longest_matches() {
    let mut guard = guard(&[("short", "abcd"), ("long", "abcdef")]);
    let mut output = Vec::new();
    let mut hits = Vec::new();
    for byte in b"xxabcdefabcd" {
        let redacted = guard.push(std::slice::from_ref(byte)).unwrap();
        hits.extend_from_slice(redacted.secret_names());
        output.extend(redacted.into_inner());
    }
    let final_chunk = guard.finish();
    hits.extend_from_slice(final_chunk.secret_names());
    output.extend(final_chunk.into_inner());
    assert_eq!(output, b"xx<secret_ref:long><secret_ref:short>");
    assert_eq!(hits, ["long", "short"]);
}

#[test]
fn arbitrary_bytes_and_partial_prefix_flush_at_eof() {
    let mut guard = guard(&[("key", "secret")]);
    assert_eq!(guard.push(b"\xffsec").unwrap().into_inner(), b"\xff");
    assert_eq!(guard.finish().into_inner(), b"sec");
    assert!(format!("{guard:?}").contains("pending_bytes"));
    assert!(!format!("{guard:?}").contains("secret"));
}

#[test]
fn duplicate_plaintext_uses_first_seed_name() {
    let mut guard = guard(&[("first", "same"), ("second", "same")]);
    let redacted = guard.push(b"same").unwrap();
    assert_eq!(redacted.secret_names(), ["first"]);
    assert_eq!(redacted.into_inner(), b"<secret_ref:first>");
}

#[test]
fn empty_seed_passes_chunks_through_without_delay() {
    let mut guard = guard(&[("ignored", "")]);
    assert_eq!(guard.push(b"plain").unwrap().into_inner(), b"plain");
    assert!(guard.finish().into_inner().is_empty());
}

#[test]
fn oversized_patterns_fail_without_exposing_content() {
    let value = "x".repeat(MAX_STREAM_SECRET_BYTES + 1);
    let error = StreamingOutputGuard::new(&[("key".into(), SecretString::from(value))])
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "secret set exceeds streaming redaction limits"
    );
    assert!(!format!("{error:?}").contains('x'));
}

#[test]
fn excessive_name_count_and_total_bytes_fail_closed() {
    let long_name = "n".repeat(MAX_STREAM_SECRET_NAME_BYTES + 1);
    assert!(
        StreamingOutputGuard::new(&[(long_name, SecretString::from("v".to_string()))]).is_err()
    );

    let too_many = (0..=MAX_STREAM_SECRET_PATTERNS)
        .map(|index| {
            (
                format!("key_{index}"),
                SecretString::from(format!("value_{index}")),
            )
        })
        .collect::<Vec<_>>();
    assert!(StreamingOutputGuard::new(&too_many).is_err());

    let count = MAX_STREAM_SECRET_TOTAL_BYTES / MAX_STREAM_SECRET_BYTES + 1;
    let total = (0..count)
        .map(|index| {
            let prefix = char::from(b'a' + index as u8);
            (
                format!("total_{index}"),
                SecretString::from(format!(
                    "{prefix}{}",
                    "x".repeat(MAX_STREAM_SECRET_BYTES - 1)
                )),
            )
        })
        .collect::<Vec<_>>();
    assert!(StreamingOutputGuard::new(&total).is_err());
}

#[test]
fn kmp_fallback_preserves_an_overlapping_match() {
    let mut guard = guard(&[("key", "ababaca")]);
    let mut output = guard.push(b"abababaca").unwrap().into_inner();
    output.extend(guard.finish().into_inner());
    assert_eq!(output, b"ab<secret_ref:key>");
}

#[test]
fn long_nonmatching_stream_compacts_consumed_bytes() {
    let secret = format!("s{}", "x".repeat(9_000));
    let mut guard =
        StreamingOutputGuard::new(&[("key".into(), SecretString::from(secret))]).unwrap();
    let input = vec![b'z'; 9_000];
    assert_eq!(guard.push(&input).unwrap().into_inner(), input);
    assert!(guard.finish().into_inner().is_empty());
}

#[test]
fn committed_input_tracks_buffered_prefix_and_eof() {
    let mut guard = guard(&[("key", "secret")]);
    assert!(
        guard
            .push(b"before-sec")
            .unwrap()
            .into_inner()
            .starts_with(b"before")
    );
    assert_eq!(guard.committed_input_bytes(), "before-".len());
    assert_eq!(
        guard.push(b"ret-after").unwrap().into_inner(),
        b"<secret_ref:key>-after"
    );
    assert_eq!(guard.committed_input_bytes(), "before-secret-after".len());
    assert!(guard.finish().into_inner().is_empty());
    assert_eq!(guard.committed_input_bytes(), "before-secret-after".len());
}

#[test]
fn empty_effective_seed_commits_passthrough_bytes() {
    let mut guard = guard(&[("ignored", "")]);
    assert_eq!(guard.push(b"plain").unwrap().into_inner(), b"plain");
    assert_eq!(guard.committed_input_bytes(), 5);
}

#[test]
fn push_after_finish_fails_closed() {
    let mut guard = guard(&[("key", "secret")]);
    let _ = guard.finish();
    assert_eq!(
        guard.push(b"later").unwrap_err(),
        StreamingOutputGuardFinished
    );
    assert!(guard.finish().into_inner().is_empty());
}
