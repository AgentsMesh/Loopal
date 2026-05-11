use loopal_agent_server::testing::parse_permission_argv;
use loopal_decision_api::DecisionMode;
use loopal_tool_api::PermissionMode;

#[test]
fn parses_valid_json() {
    let result = parse_permission_argv(r#"{"mode":"ask_dangerous","decision":"auto"}"#).unwrap();
    assert_eq!(result, (PermissionMode::AskDangerous, DecisionMode::Auto));
}

#[test]
fn parses_bypass_manual() {
    let result = parse_permission_argv(r#"{"mode":"bypass","decision":"manual"}"#).unwrap();
    assert_eq!(result, (PermissionMode::Bypass, DecisionMode::Manual));
}

#[test]
fn rejects_invalid_json() {
    let err = parse_permission_argv("ask_dangerous:auto").unwrap_err();
    assert!(err.contains("invalid permission JSON"));
}

#[test]
fn rejects_unknown_mode() {
    let err = parse_permission_argv(r#"{"mode":"banana","decision":"auto"}"#).unwrap_err();
    assert!(err.contains("invalid"));
}

#[test]
fn rejects_unknown_decision() {
    let err = parse_permission_argv(r#"{"mode":"bypass","decision":"banana"}"#).unwrap_err();
    assert!(err.contains("invalid"));
}

#[test]
fn rejects_missing_field() {
    let err = parse_permission_argv(r#"{"mode":"bypass"}"#).unwrap_err();
    assert!(err.contains("invalid"));
}
