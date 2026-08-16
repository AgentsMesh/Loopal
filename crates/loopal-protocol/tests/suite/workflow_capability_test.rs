use loopal_protocol::WorkflowAttemptCapability;

#[test]
fn attempt_capability_covers_generation_parsing_digest_and_redaction() {
    let generated = WorkflowAttemptCapability::generate();
    assert_eq!(generated.expose().len(), 64);
    assert!(WorkflowAttemptCapability::parse(generated.expose()).is_ok());
    assert_eq!(
        format!("{generated:?}"),
        "WorkflowAttemptCapability([REDACTED])"
    );

    let parsed = WorkflowAttemptCapability::parse("ab".repeat(32)).unwrap();
    assert!(WorkflowAttemptCapability::parse("1".repeat(64)).is_ok());
    assert!(parsed.matches_digest(parsed.digest()));
    assert!(
        !parsed.matches_digest(
            WorkflowAttemptCapability::parse("cd".repeat(32))
                .unwrap()
                .digest()
        )
    );

    let encoded = serde_json::to_string(&parsed).unwrap();
    assert_eq!(
        serde_json::from_str::<WorkflowAttemptCapability>(&encoded).unwrap(),
        parsed
    );
    for invalid in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        format!("{}g", "a".repeat(63)),
    ] {
        assert!(WorkflowAttemptCapability::parse(&invalid).is_err());
        assert!(
            serde_json::from_value::<WorkflowAttemptCapability>(serde_json::json!(invalid))
                .is_err()
        );
    }
    assert_eq!(
        WorkflowAttemptCapability::parse("invalid")
            .unwrap_err()
            .to_string(),
        "workflow attempt capability must be 64 lowercase hex digits"
    );
}
