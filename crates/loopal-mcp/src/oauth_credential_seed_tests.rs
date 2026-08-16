use super::{OAUTH_CREDENTIAL_ERROR, OAuthCredentialSeed};

#[test]
fn records_rotated_tokens_and_bearer_forms() {
    let seed = OAuthCredentialSeed::default();
    seed.observe(Some("first-token")).unwrap();
    seed.observe(Some("first-token")).unwrap();
    seed.observe(Some("second-token")).unwrap();

    let redactor = seed.redactor().unwrap();
    let (text, hits) =
        redactor.scan_and_redact("first-token Bearer first-token second-token Bearer second-token");
    assert!(!text.contains("first-token"));
    assert!(!text.contains("second-token"));
    assert!(hits.contains(&"mcp_oauth_access_token".into()));
    assert!(hits.contains(&"mcp_oauth_bearer".into()));
}

#[test]
fn deactivate_clears_and_rejects_late_observation() {
    let seed = OAuthCredentialSeed::default();
    seed.observe(None).unwrap();
    seed.observe(Some("")).unwrap();
    seed.observe(Some("access-token")).unwrap();
    seed.deactivate();

    assert_eq!(
        seed.observe(Some("late-token")),
        Err(OAUTH_CREDENTIAL_ERROR)
    );
    assert!(seed.redactor().is_err());
}
