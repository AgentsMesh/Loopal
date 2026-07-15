use serde_json::Value;

use crate::{ExpectedRequest, RequestRecord};

pub fn validate_request(
    expected: &ExpectedRequest,
    body: &Value,
    record: &RequestRecord,
) -> Vec<String> {
    let mut errors = Vec::new();
    if expected
        .protocol
        .as_ref()
        .is_some_and(|value| value != &record.protocol)
    {
        errors.push(format!(
            "protocol did not match {}",
            expected.protocol.as_deref().unwrap_or("")
        ));
    }
    if expected
        .model
        .as_ref()
        .is_some_and(|value| value != &record.model)
    {
        errors.push(format!(
            "model did not match {}",
            expected.model.as_deref().unwrap_or("")
        ));
    }
    if expected
        .user_contains
        .as_ref()
        .is_some_and(|value| !record.last_user_text.contains(value))
    {
        errors.push("last user message did not contain expected text".into());
    }
    let encoded = body.to_string();
    if expected
        .body_contains
        .as_ref()
        .is_some_and(|value| !encoded.contains(value))
    {
        errors.push("request body did not contain expected text".into());
    }
    if expected
        .body_excludes
        .as_ref()
        .is_some_and(|value| encoded.contains(value))
    {
        errors.push("request body contained excluded text".into());
    }
    if expected
        .tool_result_id
        .as_ref()
        .is_some_and(|value| !record.tool_result_ids.contains(value))
    {
        errors.push("request did not contain expected tool result id".into());
    }
    if expected
        .min_tools
        .is_some_and(|value| record.tool_count < value)
    {
        errors.push(format!(
            "expected at least {} tools",
            expected.min_tools.unwrap_or(0)
        ));
    }
    if expected
        .message_count
        .is_some_and(|value| record.message_count != value)
    {
        errors.push(format!(
            "expected {} messages",
            expected.message_count.unwrap_or(0)
        ));
    }
    if expected
        .assistant_block_types
        .as_ref()
        .is_some_and(|value| value != &record.assistant_block_types)
    {
        errors.push("assistant block types did not match expected order".into());
    }
    if expected
        .server_block_count
        .is_some_and(|value| value != record.server_block_count)
    {
        errors.push(format!(
            "expected {} server blocks",
            expected.server_block_count.unwrap_or(0)
        ));
    }
    if expected
        .thinking_enabled
        .is_some_and(|value| record.thinking_enabled != value)
    {
        errors.push(format!(
            "expected thinking enabled to be {}",
            expected.thinking_enabled.unwrap_or(false)
        ));
    }
    if expected
        .image_block_count
        .is_some_and(|value| record.image_block_count != value)
    {
        errors.push(format!(
            "expected {} image blocks",
            expected.image_block_count.unwrap_or(0)
        ));
    }
    errors
}
