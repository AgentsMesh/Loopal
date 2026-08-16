use secrecy::SecretString;

use super::SecretProcessOutputSanitizer;

#[test]
fn oversized_secret_seed_fails_with_content_free_error() {
    let canary = "x".repeat(loopal_output_guard::MAX_STREAM_SECRET_BYTES + 1);
    let seed = vec![("token".into(), SecretString::from(canary.clone()))];

    let error = match SecretProcessOutputSanitizer::new("Bash", "session", &seed) {
        Ok(_) => panic!("oversized seed must fail"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(message.contains("streaming redaction limits"));
    assert!(!message.contains(&canary));
}
