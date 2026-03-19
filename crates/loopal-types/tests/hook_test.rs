use loopal_types::hook::{HookConfig, HookEvent, HookResult};

#[test]
fn test_hook_result_is_success_true() {
    let result = HookResult {
        exit_code: 0,
        stdout: "ok".into(),
        stderr: String::new(),
    };
    assert!(result.is_success());
}

#[test]
fn test_hook_result_is_success_false_for_one() {
    let result = HookResult {
        exit_code: 1,
        stdout: String::new(),
        stderr: "error".into(),
    };
    assert!(!result.is_success());
}

#[test]
fn test_hook_result_is_success_false_for_negative() {
    let result = HookResult {
        exit_code: -1,
        stdout: String::new(),
        stderr: "signal".into(),
    };
    assert!(!result.is_success());
}

#[test]
fn test_hook_result_is_success_false_for_127() {
    let result = HookResult {
        exit_code: 127,
        stdout: String::new(),
        stderr: "command not found".into(),
    };
    assert!(!result.is_success());
}

#[test]
fn test_hook_config_default_timeout() {
    let json = r#"{
        "event": "pre_tool_use",
        "command": "echo hello"
    }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.timeout_ms, 10_000);
    assert_eq!(config.event, HookEvent::PreToolUse);
    assert_eq!(config.command, "echo hello");
    assert!(config.tool_filter.is_none());
}

#[test]
fn test_hook_config_custom_timeout() {
    let json = r#"{
        "event": "post_tool_use",
        "command": "lint.sh",
        "timeout_ms": 30000
    }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.timeout_ms, 30_000);
}

#[test]
fn test_hook_config_with_tool_filter() {
    let json = r#"{
        "event": "pre_tool_use",
        "command": "check.sh",
        "tool_filter": ["Bash", "Write"]
    }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let filter = config.tool_filter.unwrap();
    assert_eq!(filter, vec!["Bash", "Write"]);
}

#[test]
fn test_hook_event_serde_roundtrip_pre_tool_use() {
    let event = HookEvent::PreToolUse;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, HookEvent::PreToolUse);
}

#[test]
fn test_hook_event_serde_roundtrip_post_tool_use() {
    let event = HookEvent::PostToolUse;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, HookEvent::PostToolUse);
}

#[test]
fn test_hook_event_serde_roundtrip_pre_request() {
    let event = HookEvent::PreRequest;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, HookEvent::PreRequest);
}

#[test]
fn test_hook_event_serde_roundtrip_post_input() {
    let event = HookEvent::PostInput;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, HookEvent::PostInput);
}

#[test]
fn test_hook_event_serialized_names() {
    assert_eq!(
        serde_json::to_string(&HookEvent::PreToolUse).unwrap(),
        "\"pre_tool_use\""
    );
    assert_eq!(
        serde_json::to_string(&HookEvent::PostToolUse).unwrap(),
        "\"post_tool_use\""
    );
    assert_eq!(
        serde_json::to_string(&HookEvent::PreRequest).unwrap(),
        "\"pre_request\""
    );
    assert_eq!(
        serde_json::to_string(&HookEvent::PostInput).unwrap(),
        "\"post_input\""
    );
}

#[test]
fn test_hook_config_serde_roundtrip() {
    let config = HookConfig {
        event: HookEvent::PostToolUse,
        command: "cargo fmt".to_string(),
        tool_filter: Some(vec!["Write".to_string()]),
        timeout_ms: 5000,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: HookConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event, config.event);
    assert_eq!(deserialized.command, config.command);
    assert_eq!(deserialized.tool_filter, config.tool_filter);
    assert_eq!(deserialized.timeout_ms, config.timeout_ms);
}
