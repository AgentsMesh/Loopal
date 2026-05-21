use loopal_tool_api::TypedTool;
use loopal_tool_idle::{
    MAX_IDLE_DURATION_SECS, MIN_IDLE_DURATION_SECS, RequestIdleParams, RequestIdleTool,
    validate_duration,
};

#[test]
fn name_is_request_idle() {
    assert_eq!(RequestIdleTool.name(), "request_idle");
}

#[test]
fn description_mentions_deadline_and_no_goal_mutation() {
    let d = RequestIdleTool.description();
    assert!(d.contains("max_idle_duration_secs"));
    assert!(d.contains("NOT change the goal"));
    assert!(d.contains("60..=86400"));
}

#[test]
fn validate_duration_rejects_zero() {
    assert!(validate_duration(0).is_err());
}

#[test]
fn validate_duration_rejects_below_minimum() {
    assert!(validate_duration(MIN_IDLE_DURATION_SECS - 1).is_err());
}

#[test]
fn validate_duration_accepts_minimum() {
    assert!(validate_duration(MIN_IDLE_DURATION_SECS).is_ok());
}

#[test]
fn validate_duration_accepts_one_hour() {
    assert!(validate_duration(3600).is_ok());
}

#[test]
fn validate_duration_accepts_maximum() {
    assert!(validate_duration(MAX_IDLE_DURATION_SECS).is_ok());
}

#[test]
fn validate_duration_rejects_above_maximum() {
    let err = validate_duration(MAX_IDLE_DURATION_SECS + 1).unwrap_err();
    assert!(err.contains("infeasible"));
}

#[test]
fn parse_minimal_request_with_required_fields() {
    let json = r#"{
        "reason": "no actionable next step until cron fires",
        "max_idle_duration_secs": 1800
    }"#;
    let params: RequestIdleParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.max_idle_duration_secs, 1800);
    assert!(params.expected_wake_signal.is_none());
}

#[test]
fn parse_request_with_optional_wake_signal() {
    let json = r#"{
        "reason": "waiting for next cron",
        "max_idle_duration_secs": 600,
        "expected_wake_signal": "next cron fire at :15"
    }"#;
    let params: RequestIdleParams = serde_json::from_str(json).unwrap();
    assert_eq!(
        params.expected_wake_signal.as_deref(),
        Some("next cron fire at :15")
    );
}
