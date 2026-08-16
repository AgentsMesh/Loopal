use loopal_tui::views::permission_inline;
use loopal_view_state::PendingPermission;

use crate::inline_render_test::render_to_buffer;

#[test]
fn renders_tool_name_and_keys() {
    let permission = PendingPermission {
        id: "1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"cmd": "ls"}),
        intent_digest: None,
        cursor: Default::default(),
    };
    let output = render_to_buffer(60, 6, |frame, area| {
        permission_inline::render_prepared(
            frame,
            &permission_inline::prepare(&permission),
            area,
            None,
        )
    });
    assert!(output.contains("⚠ Tool: Bash"));
    assert!(output.contains("Allow"));
    assert!(output.contains("Deny"));
    assert!(output.contains("[y]"));
    assert!(output.contains("[n]"));
    assert!(output.contains("Enter confirm"));
}

#[test]
fn truncates_large_input_with_ellipsis() {
    let mut input = serde_json::Map::new();
    for index in 0..20 {
        input.insert(format!("k{index}"), serde_json::json!(index));
    }
    let permission = PendingPermission {
        id: "1".into(),
        name: "X".into(),
        input: serde_json::Value::Object(input),
        intent_digest: None,
        cursor: Default::default(),
    };
    let output = render_to_buffer(80, 12, |frame, area| {
        permission_inline::render_prepared(
            frame,
            &permission_inline::prepare(&permission),
            area,
            None,
        )
    });
    assert!(output.contains("more lines"));
}

#[test]
fn simple_input_has_compact_height() {
    let permission = PendingPermission {
        id: "1".into(),
        name: "X".into(),
        input: serde_json::json!({}),
        intent_digest: None,
        cursor: Default::default(),
    };
    assert_eq!(permission_inline::height(&permission, 80), 3);
}
