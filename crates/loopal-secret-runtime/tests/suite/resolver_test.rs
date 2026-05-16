use std::collections::HashMap;

use loopal_secret_runtime::{collect_wire_refs, resolve_in_value};
use secrecy::SecretString;
use serde_json::json;

fn build_secrets(pairs: &[(&str, &str)]) -> HashMap<String, SecretString> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), SecretString::from((*v).to_string())))
        .collect()
}

#[test]
fn collect_only_scans_whitelisted_fields() {
    let v = json!({
        "command": "curl -H 'Bearer <secret_ref:hf>'",
        "description": "this <secret_ref:other> is NOT whitelisted"
    });
    let names = collect_wire_refs(&v, &["command"]);
    assert_eq!(names, vec!["hf".to_string()]);
}

#[test]
fn collect_descends_into_nested_objects() {
    let v = json!({
        "env": {
            "OPENAI_KEY": "<secret_ref:openai>",
            "OTHER": "plain"
        }
    });
    let names = collect_wire_refs(&v, &["env"]);
    assert_eq!(names, vec!["openai".to_string()]);
}

#[test]
fn resolve_substitutes_whitelisted_only() {
    let mut v = json!({
        "command": "curl -H 'Bearer <secret_ref:hf>'",
        "description": "ignore <secret_ref:hf>"
    });
    let secrets = build_secrets(&[("hf", "hf-tokenvalue")]);
    let report = resolve_in_value(&mut v, &secrets, &["command"]);

    assert_eq!(v["command"], json!("curl -H 'Bearer hf-tokenvalue'"));
    assert_eq!(v["description"], json!("ignore <secret_ref:hf>"));
    assert_eq!(report.resolved_names, vec!["hf".to_string()]);
}

#[test]
fn missing_secret_becomes_missing_placeholder() {
    let mut v = json!({ "command": "echo <secret_ref:ghost>" });
    let secrets = build_secrets(&[]);
    let report = resolve_in_value(&mut v, &secrets, &["command"]);
    assert_eq!(v["command"], json!("echo <missing-secret:ghost>"));
    assert_eq!(report.missing, vec!["ghost".to_string()]);
}

#[test]
fn same_name_referenced_twice_deduplicated_in_report() {
    let mut v = json!({ "command": "A=<secret_ref:hf> B=<secret_ref:hf>" });
    let secrets = build_secrets(&[("hf", "tokenvalue")]);
    let report = resolve_in_value(&mut v, &secrets, &["command"]);
    assert_eq!(report.resolved_names, vec!["hf".to_string()]);
    assert_eq!(v["command"], json!("A=tokenvalue B=tokenvalue"));
}

#[test]
fn nested_env_object_resolved_recursively() {
    let mut v = json!({
        "env": {
            "OPENAI_KEY": "<secret_ref:openai>",
            "HF": "<secret_ref:hf>"
        }
    });
    let secrets = build_secrets(&[("openai", "sk-abc12345"), ("hf", "hf-12345678")]);
    let report = resolve_in_value(&mut v, &secrets, &["env"]);

    assert_eq!(v["env"]["OPENAI_KEY"], json!("sk-abc12345"));
    assert_eq!(v["env"]["HF"], json!("hf-12345678"));
    let mut names = report.resolved_names.clone();
    names.sort();
    assert_eq!(names, vec!["hf".to_string(), "openai".to_string()]);
}

#[test]
fn non_string_values_untouched() {
    let mut v = json!({
        "command": "echo hi",
        "timeout": 30,
        "run_in_background": false
    });
    let secrets = build_secrets(&[]);
    let report = resolve_in_value(&mut v, &secrets, &["command"]);
    assert_eq!(v["timeout"], json!(30));
    assert_eq!(v["run_in_background"], json!(false));
    assert!(report.resolved_names.is_empty());
    assert!(report.missing.is_empty());
}

#[test]
fn empty_whitelist_does_nothing() {
    let mut v = json!({ "command": "<secret_ref:hf>" });
    let secrets = build_secrets(&[("hf", "hf-12345678")]);
    let report = resolve_in_value(&mut v, &secrets, &[]);
    assert_eq!(v["command"], json!("<secret_ref:hf>"));
    assert!(report.resolved_names.is_empty());
}

#[test]
fn malformed_placeholder_passes_through() {
    let mut v = json!({ "command": "<SECRET_REF:hf> <secret_ref:Bad>" });
    let secrets = build_secrets(&[("hf", "hf-12345678")]);
    let report = resolve_in_value(&mut v, &secrets, &["command"]);
    assert_eq!(v["command"], json!("<SECRET_REF:hf> <secret_ref:Bad>"));
    assert!(report.resolved_names.is_empty());
}
