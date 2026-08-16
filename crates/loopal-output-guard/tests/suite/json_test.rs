use loopal_output_guard::{JsonGuardError, OutputGuard};
use secrecy::SecretString;
use serde_json::json;

fn guard() -> OutputGuard {
    OutputGuard::new(&[
        (
            "token".to_string(),
            SecretString::from("secret-token".to_string()),
        ),
        ("pin".to_string(), SecretString::from("42".to_string())),
    ])
    .unwrap()
}

#[test]
fn recursively_redacts_nested_json_strings_only() {
    let input = json!({
        "outer": ["secret-token", {"inner": "x42y", "number": 42}],
        "key-secret-token": "clean",
        "clean": true,
        "null": null
    });
    let guarded = guard().guard_json(&input, 256).unwrap();
    assert_eq!(guarded.secret_names(), &["token", "pin"]);
    let debug = format!("{guarded:?}");
    assert!(debug.contains("secret_count"));
    assert!(!debug.contains("secret-token"));
    let output = guarded.into_inner();
    assert_eq!(
        output.value(),
        &json!({
            "outer": ["<secret_ref:token>", {"inner": "x<secret_ref:pin>y", "number": 42}],
            "key-<secret_ref:token>": "clean",
            "clean": true,
            "null": null
        })
    );
    assert_eq!(
        output.encoded_bytes(),
        serde_json::to_vec(output.value()).unwrap().len()
    );
}

#[test]
fn encoded_bound_counts_json_escaping_bytes() {
    let input = json!({"value": "\n"});
    let exact = serde_json::to_vec(&input).unwrap().len();
    assert!(guard().guard_json(&input, exact).is_ok());
    assert_eq!(
        guard().guard_json(&input, exact - 1),
        Err(JsonGuardError::EncodedByteLimitExceeded {
            actual_bytes: exact,
            max_bytes: exact - 1,
        })
    );
}

#[test]
fn json_size_errors_and_debug_output_hide_secrets() {
    let secret = "secret-token";
    let error = guard()
        .guard_json(&json!({"value": secret}), 1)
        .unwrap_err();
    assert!(!format!("{error}").contains(secret));
    assert!(!format!("{error:?}").contains(secret));

    let guarded = guard()
        .guard_json(&json!({"value": secret}), 128)
        .unwrap()
        .into_inner();
    assert!(!format!("{guarded:?}").contains(secret));
}

#[test]
fn key_collision_after_redaction_is_rejected() {
    let error = guard()
        .guard_json(&json!({"secret-token": 1, "<secret_ref:token>": 2}), 128)
        .unwrap_err();
    assert_eq!(error, JsonGuardError::RedactedKeyCollision);
}

#[test]
fn guarded_json_returns_an_owned_redacted_value() {
    let input = json!(["secret-token"]);
    let output = guard().guard_json(&input, 64).unwrap().into_inner();
    assert_eq!(output.into_value(), json!(["<secret_ref:token>"]));
    assert_eq!(input, json!(["secret-token"]));
}
