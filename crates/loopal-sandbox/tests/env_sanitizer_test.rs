use std::collections::HashMap;

use loopal_sandbox::env_sanitizer::{find_sensitive_vars, is_sensitive, sanitize_env};

#[test]
fn safe_vars_kept() {
    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin".to_string());
    env.insert("HOME".to_string(), "/home/user".to_string());
    env.insert("TERM".to_string(), "xterm".to_string());

    let clean = sanitize_env(&env);
    assert_eq!(clean.get("PATH"), Some(&"/usr/bin".to_string()));
    assert_eq!(clean.get("HOME"), Some(&"/home/user".to_string()));
    assert_eq!(clean.get("TERM"), Some(&"xterm".to_string()));
}

#[test]
fn sensitive_vars_removed() {
    let mut env = HashMap::new();
    env.insert("AWS_SECRET_ACCESS_KEY".to_string(), "sk-xxx".to_string());
    env.insert("GITHUB_TOKEN".to_string(), "ghp_xxx".to_string());
    env.insert("DATABASE_URL".to_string(), "postgres://...".to_string());
    env.insert("PATH".to_string(), "/usr/bin".to_string());

    let clean = sanitize_env(&env);
    assert!(!clean.contains_key("AWS_SECRET_ACCESS_KEY"));
    assert!(!clean.contains_key("GITHUB_TOKEN"));
    assert!(!clean.contains_key("DATABASE_URL"));
    assert!(clean.contains_key("PATH"));
}

#[test]
fn is_sensitive_case_insensitive() {
    assert!(is_sensitive("aws_secret_key"));
    assert!(is_sensitive("AWS_SECRET_KEY"));
    assert!(is_sensitive("my_api_key_here"));
}

#[test]
fn is_sensitive_safe_list_overrides() {
    assert!(!is_sensitive("PATH"));
    assert!(!is_sensitive("HOME"));
    assert!(!is_sensitive("LOOPAL_MODEL"));
}

#[test]
fn find_sensitive_vars_returns_matches() {
    let mut env = HashMap::new();
    env.insert("SAFE_VAR".to_string(), "ok".to_string());
    env.insert("STRIPE_SECRET_KEY".to_string(), "sk_xxx".to_string());
    env.insert("OPENAI_API_KEY".to_string(), "sk-xxx".to_string());

    let found = find_sensitive_vars(&env);
    assert!(found.contains(&"STRIPE_SECRET_KEY".to_string()));
    assert!(found.contains(&"OPENAI_API_KEY".to_string()));
    assert!(!found.contains(&"SAFE_VAR".to_string()));
}

#[test]
fn empty_env_returns_empty() {
    let env = HashMap::new();
    let clean = sanitize_env(&env);
    assert!(clean.is_empty());
}

#[test]
fn unrecognized_vars_kept() {
    let mut env = HashMap::new();
    env.insert("MY_CUSTOM_VAR".to_string(), "value".to_string());
    env.insert("CARGO_HOME".to_string(), "/home/.cargo".to_string());

    let clean = sanitize_env(&env);
    assert!(clean.contains_key("MY_CUSTOM_VAR"));
    assert!(clean.contains_key("CARGO_HOME"));
}
