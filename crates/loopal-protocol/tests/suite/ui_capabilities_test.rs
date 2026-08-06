use loopal_protocol::{UiCapabilities, UiCapability};

#[test]
fn omitted_capabilities_fail_closed() {
    let capabilities: UiCapabilities = serde_json::from_value(serde_json::json!({})).unwrap();

    assert_eq!(capabilities, UiCapabilities::NONE);
    assert!(!capabilities.supports(UiCapability::Permission));
    assert!(!capabilities.supports(UiCapability::Question));
    assert!(!capabilities.supports(UiCapability::PlanApproval));
}

#[test]
fn capabilities_round_trip_as_extensible_object() {
    let capabilities: UiCapabilities = serde_json::from_value(serde_json::json!({
        "permission": true,
        "question": false,
        "plan_approval": true,
        "future_capability": true
    }))
    .unwrap();

    assert!(capabilities.supports(UiCapability::Permission));
    assert!(!capabilities.supports(UiCapability::Question));
    assert!(capabilities.supports(UiCapability::PlanApproval));
    assert_eq!(
        serde_json::to_value(capabilities).unwrap(),
        serde_json::json!({
            "permission": true,
            "question": false,
            "plan_approval": true
        })
    );
}
