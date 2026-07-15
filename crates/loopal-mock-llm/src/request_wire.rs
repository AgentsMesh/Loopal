use serde_json::Value;

use crate::protocol::WireProtocol;
use crate::request::CanonicalRequest;

pub(crate) fn canonicalize(
    protocol: WireProtocol,
    body: &Value,
    route_model: Option<&str>,
) -> CanonicalRequest {
    match protocol {
        WireProtocol::Anthropic => anthropic(body),
        WireProtocol::OpenAiResponses => responses(body),
        WireProtocol::OpenAiCompat => crate::request_wire_alt::compat(body),
        WireProtocol::Google => {
            crate::request_wire_alt::google(body, route_model.unwrap_or_default())
        }
    }
}

fn anthropic(body: &Value) -> CanonicalRequest {
    let messages = array(body, "messages");
    let tools = array(body, "tools");
    CanonicalRequest {
        model: string(body, "model"),
        message_count: messages.len(),
        tool_names: tools.iter().filter_map(|v| text(v, "name")).collect(),
        tool_result_ids: messages.iter().flat_map(anthropic_result_ids).collect(),
        tool_result_error_ids: messages
            .iter()
            .flat_map(anthropic_error_result_ids)
            .collect(),
        assistant_block_types: messages
            .iter()
            .filter(|m| m["role"] == "assistant")
            .flat_map(content_types)
            .collect(),
        server_block_count: messages
            .iter()
            .flat_map(content_types)
            .filter(|t| t == "server_tool_use" || t.ends_with("_tool_result"))
            .count(),
        image_block_count: anthropic_image_count(messages),
        last_user_text: last_text(messages, "user", anthropic_text),
        has_system: body.get("system").is_some(),
        thinking_enabled: body.get("thinking").is_some(),
        stream: body["stream"].as_bool().unwrap_or(false),
        max_tokens: body["max_tokens"].as_u64().unwrap_or(0),
    }
}

fn responses(body: &Value) -> CanonicalRequest {
    let input = array(body, "input");
    let tools = array(body, "tools");
    CanonicalRequest {
        model: string(body, "model"),
        message_count: input.iter().filter(|v| v["type"] == "message").count(),
        tool_names: tools
            .iter()
            .filter_map(|v| text(v, "name").or_else(|| text(v, "type")))
            .collect(),
        tool_result_ids: input
            .iter()
            .filter(|v| v["type"] == "function_call_output")
            .filter_map(|v| text(v, "call_id"))
            .collect(),
        tool_result_error_ids: input
            .iter()
            .filter(|v| {
                v["type"] == "function_call_output"
                    && v["output"]
                        .as_str()
                        .is_some_and(|output| output.starts_with("[error]"))
            })
            .filter_map(|v| text(v, "call_id"))
            .collect(),
        assistant_block_types: input
            .iter()
            .filter(|v| v["role"] == "assistant" || v["type"] != "message")
            .filter_map(|v| text(v, "type"))
            .collect(),
        server_block_count: input
            .iter()
            .filter(|v| v["type"] == "web_search_call")
            .count(),
        image_block_count: nested_type_count(input, "input_image"),
        last_user_text: last_text(input, "user", responses_text),
        has_system: body.get("instructions").is_some(),
        thinking_enabled: body.get("reasoning").is_some(),
        stream: body["stream"].as_bool().unwrap_or(false),
        max_tokens: body["max_output_tokens"].as_u64().unwrap_or(0),
    }
}

pub(crate) fn array<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value[field].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub(crate) fn string(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().into()
}
pub(crate) fn text(value: &Value, field: &str) -> Option<String> {
    value[field].as_str().map(str::to_owned)
}
fn content_types(value: &Value) -> Vec<String> {
    array(value, "content")
        .iter()
        .filter_map(|v| text(v, "type"))
        .collect()
}
fn anthropic_result_ids(value: &Value) -> Vec<String> {
    array(value, "content")
        .iter()
        .filter(|v| v["type"] == "tool_result")
        .filter_map(|v| text(v, "tool_use_id"))
        .collect()
}
fn anthropic_error_result_ids(value: &Value) -> Vec<String> {
    array(value, "content")
        .iter()
        .filter(|v| v["type"] == "tool_result" && v["is_error"] == true)
        .filter_map(|v| text(v, "tool_use_id"))
        .collect()
}
fn anthropic_image_count(messages: &[Value]) -> usize {
    messages
        .iter()
        .flat_map(|message| array(message, "content"))
        .map(|block| {
            usize::from(block["type"] == "image")
                + if block["type"] == "tool_result" {
                    array(block, "content")
                        .iter()
                        .filter(|item| item["type"] == "image")
                        .count()
                } else {
                    0
                }
        })
        .sum()
}
pub(crate) fn last_text(items: &[Value], role: &str, get: fn(&Value) -> String) -> String {
    items
        .iter()
        .rev()
        .find(|v| v["role"] == role)
        .map(get)
        .unwrap_or_default()
}
fn anthropic_text(value: &Value) -> String {
    value["content"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            array(value, "content")
                .iter()
                .filter_map(|v| v["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
}
pub(crate) fn responses_text(value: &Value) -> String {
    array(value, "content")
        .iter()
        .filter(|v| v["type"] == "input_text")
        .filter_map(|v| v["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
pub(crate) fn nested_type_count(items: &[Value], kind: &str) -> usize {
    items
        .iter()
        .map(|v| {
            array(v, "content")
                .iter()
                .filter(|p| p["type"] == kind)
                .count()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn anthropic_counts_direct_and_tool_result_images() {
        let messages = json!({"messages": [{"role": "user", "content": [
            {"type": "image", "source": {}},
            {"type": "tool_result", "content": [{"type": "image", "source": {}}]}
        ]}]});
        assert_eq!(super::anthropic(&messages).image_block_count, 2);
    }
}
