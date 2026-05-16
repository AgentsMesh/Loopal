//! `detect_argv_exposure` flags secrets that were substituted into
//! shell-argv-visible fields (`command`). The runtime warns + audits when
//! this happens because the recommended path is the `env` field.

use loopal_secret_runtime::detect_argv_exposure;
use loopal_vault_api::SecretString;
use serde_json::json;

fn seed(pairs: &[(&str, &str)]) -> Vec<(String, SecretString)> {
    pairs
        .iter()
        .map(|(n, v)| ((*n).to_string(), SecretString::from((*v).to_string())))
        .collect()
}

#[test]
fn detects_secret_substituted_into_command_field() {
    let input = json!({ "command": "curl -H 'Bearer sk-12345abc'" });
    let leaked = detect_argv_exposure(&input, &seed(&[("api_key", "sk-12345abc")]));
    assert_eq!(leaked, vec!["api_key"]);
}

#[test]
fn ignores_secret_only_in_env_field() {
    // The recommended channel — `env` is NOT considered argv-visible.
    let input = json!({
        "command": "echo $TOKEN",
        "env": { "TOKEN": "sk-12345abc" }
    });
    let leaked = detect_argv_exposure(&input, &seed(&[("api_key", "sk-12345abc")]));
    assert!(
        leaked.is_empty(),
        "env should not be flagged, got {leaked:?}"
    );
}

#[test]
fn empty_seed_returns_empty() {
    let input = json!({ "command": "echo hello" });
    assert!(detect_argv_exposure(&input, &[]).is_empty());
}

#[test]
fn no_command_field_returns_empty() {
    let input = json!({ "url": "https://example.com" });
    let leaked = detect_argv_exposure(&input, &seed(&[("k", "sk-12345abc")]));
    assert!(leaked.is_empty());
}

#[test]
fn dedups_same_secret_appearing_multiple_times_in_command() {
    let input = json!({ "command": "echo sk-12345abc; cat sk-12345abc; ls" });
    let leaked = detect_argv_exposure(&input, &seed(&[("k", "sk-12345abc")]));
    assert_eq!(leaked, vec!["k"]);
}

#[test]
fn detects_multiple_distinct_secrets_in_command() {
    let input = json!({ "command": "curl -u user:sk-aaa1234 -H 'X: hf-bbb5678'" });
    let mut leaked =
        detect_argv_exposure(&input, &seed(&[("a", "sk-aaa1234"), ("b", "hf-bbb5678")]));
    leaked.sort();
    assert_eq!(leaked, vec!["a", "b"]);
}

#[test]
fn returns_empty_when_command_does_not_contain_secret() {
    let input = json!({ "command": "echo hello world" });
    let leaked = detect_argv_exposure(&input, &seed(&[("k", "sk-not-present-12")]));
    assert!(leaked.is_empty());
}
