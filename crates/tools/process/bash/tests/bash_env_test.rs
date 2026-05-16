use std::collections::HashMap;

use loopal_tool_api::TypedTool;
use loopal_tool_bash::{BashParams, BashTool};

use super::make_store;

fn params_with_env(env: HashMap<String, String>) -> BashParams {
    BashParams {
        command: "echo".to_string(),
        timeout: None,
        run_in_background: None,
        description: None,
        env: Some(env),
    }
}

fn one_env(k: &str, v: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(k.to_string(), v.to_string());
    m
}

#[test]
fn precheck_accepts_valid_env_keys() {
    let tool = BashTool::new(make_store());
    let mut env = HashMap::new();
    env.insert("FOO".to_string(), "12345678".to_string());
    env.insert("BAR_BAZ_1".to_string(), "abcdefgh".to_string());
    assert!(tool.precheck(&params_with_env(env)).is_none());
}

#[test]
fn precheck_rejects_lowercase_env_key() {
    let tool = BashTool::new(make_store());
    let reason = tool
        .precheck(&params_with_env(one_env("foo", "12345678")))
        .unwrap();
    assert!(reason.contains("^[A-Z_][A-Z0-9_]*$"), "actual: {reason}");
}

#[test]
fn precheck_rejects_leading_digit_env_key() {
    let tool = BashTool::new(make_store());
    assert!(
        tool.precheck(&params_with_env(one_env("1FOO", "12345678")))
            .is_some()
    );
}

#[test]
fn precheck_rejects_path_override() {
    let tool = BashTool::new(make_store());
    let reason = tool
        .precheck(&params_with_env(one_env("PATH", "/usr/bin")))
        .unwrap();
    assert!(reason.contains("blacklisted"), "actual: {reason}");
}

#[test]
fn precheck_rejects_ld_preload_override() {
    let tool = BashTool::new(make_store());
    assert!(
        tool.precheck(&params_with_env(one_env("LD_PRELOAD", "/tmp/evil.so")))
            .is_some()
    );
}

#[test]
fn precheck_rejects_dyld_overrides() {
    let tool = BashTool::new(make_store());
    for key in &[
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
        "HOME",
    ] {
        assert!(
            tool.precheck(&params_with_env(one_env(key, "/tmp/foo")))
                .is_some(),
            "should reject blacklist key {key}"
        );
    }
}

#[test]
fn precheck_allows_underscore_only_key() {
    let tool = BashTool::new(make_store());
    assert!(
        tool.precheck(&params_with_env(one_env("_PRIVATE", "12345678")))
            .is_none()
    );
}

#[test]
fn precheck_rejects_dash_in_key() {
    let tool = BashTool::new(make_store());
    assert!(
        tool.precheck(&params_with_env(one_env("FOO-BAR", "12345678")))
            .is_some()
    );
}

#[test]
fn precheck_no_env_field_is_fine() {
    let tool = BashTool::new(make_store());
    let p = BashParams {
        command: "ls".to_string(),
        timeout: None,
        run_in_background: None,
        description: None,
        env: None,
    };
    assert!(tool.precheck(&p).is_none());
}
