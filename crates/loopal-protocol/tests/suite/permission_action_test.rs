use loopal_protocol::{
    calculate_permission_action_digest, calculate_permission_display_digest,
    calculate_permission_schema_digest,
};
use serde_json::json;

#[test]
fn canonical_digests_cover_every_json_variant_and_key_order() {
    let first = json!({
        "z": [null, true, false, 3, "quoted\ntext"],
        "a": {"second": 2, "first": 1},
    });
    let reordered = json!({
        "a": {"first": 1, "second": 2},
        "z": [null, true, false, 3, "quoted\ntext"],
    });

    assert_eq!(
        calculate_permission_action_digest("call", "Bash", &first),
        calculate_permission_action_digest("call", "Bash", &reordered),
    );
    assert_eq!(
        calculate_permission_display_digest(&first),
        calculate_permission_display_digest(&reordered),
    );
    assert_eq!(
        calculate_permission_schema_digest(&first),
        calculate_permission_schema_digest(&reordered),
    );
}

#[test]
fn action_digest_binds_call_and_tool_identity() {
    let input = json!({"command": "pwd"});
    let digest = calculate_permission_action_digest("call-1", "Bash", &input);

    assert_ne!(
        digest,
        calculate_permission_action_digest("call-2", "Bash", &input)
    );
    assert_ne!(
        digest,
        calculate_permission_action_digest("call-1", "Read", &input)
    );
}
