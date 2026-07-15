use serde_json::Value;

use crate::request::CanonicalRequest;
use crate::request_wire::{array, last_text, nested_type_count, string, text};

pub(crate) fn compat(body: &Value) -> CanonicalRequest {
    let messages = array(body, "messages");
    let tools = array(body, "tools");
    CanonicalRequest {
        model: string(body, "model"),
        message_count: messages.iter().filter(|m| m["role"] != "system").count(),
        tool_names: tools
            .iter()
            .filter_map(|v| text(&v["function"], "name"))
            .collect(),
        tool_result_ids: messages
            .iter()
            .filter(|m| m["role"] == "tool")
            .filter_map(|m| text(m, "tool_call_id"))
            .collect(),
        tool_result_error_ids: Vec::new(),
        assistant_block_types: compat_assistant_types(messages),
        server_block_count: 0,
        image_block_count: nested_type_count(messages, "image_url"),
        last_user_text: last_text(messages, "user", compat_text),
        has_system: messages.iter().any(|m| m["role"] == "system"),
        thinking_enabled: body.get("reasoning_effort").is_some(),
        stream: body["stream"].as_bool().unwrap_or(false),
        max_tokens: body["max_completion_tokens"].as_u64().unwrap_or(0),
    }
}

pub(crate) fn google(body: &Value, model: &str) -> CanonicalRequest {
    let contents = array(body, "contents");
    let tools = array(body, "tools");
    let assistant_block_types: Vec<String> = contents
        .iter()
        .filter(|value| value["role"] == "model")
        .flat_map(|value| google_part_types(&value["parts"]))
        .collect();
    CanonicalRequest {
        model: model.into(),
        message_count: contents.len(),
        tool_names: tools.iter().flat_map(google_tool_names).collect(),
        tool_result_ids: Vec::new(),
        tool_result_error_ids: Vec::new(),
        server_block_count: assistant_block_types
            .iter()
            .filter(|kind| kind.starts_with("server_tool_"))
            .count(),
        assistant_block_types,
        image_block_count: nested_key_count(contents, "inlineData"),
        last_user_text: last_text(contents, "user", google_text),
        has_system: body.get("systemInstruction").is_some(),
        thinking_enabled: body["generationConfig"].get("thinkingConfig").is_some(),
        stream: true,
        max_tokens: body["generationConfig"]["maxOutputTokens"]
            .as_u64()
            .unwrap_or(0),
    }
}

fn compat_text(value: &Value) -> String {
    value["content"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            array(value, "content")
                .iter()
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
}

fn google_text(value: &Value) -> String {
    array(value, "parts")
        .iter()
        .filter_map(|v| v["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn nested_key_count(items: &[Value], key: &str) -> usize {
    items
        .iter()
        .map(|v| {
            array(v, "parts")
                .iter()
                .filter(|p| p.get(key).is_some())
                .count()
        })
        .sum()
}

fn compat_assistant_types(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter(|v| v["role"] == "assistant")
        .flat_map(|v| {
            let mut out = Vec::new();
            if v["reasoning_content"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
            {
                out.push("thinking".into());
            }
            if v.get("content").is_some() {
                out.push("text".into());
            }
            out.extend(array(v, "tool_calls").iter().map(|_| "tool_use".into()));
            out
        })
        .collect()
}

fn google_tool_names(value: &Value) -> Vec<String> {
    let mut names: Vec<String> = array(value, "functionDeclarations")
        .iter()
        .filter_map(|v| text(v, "name"))
        .collect();
    if value.get("googleSearch").is_some() {
        names.push("WebSearch".into());
    }
    names
}

fn google_part_types(parts: &Value) -> Vec<String> {
    parts
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| {
            if part.get("functionCall").is_some() {
                Some("tool_use".into())
            } else if part["thought"] == true {
                Some("thinking".into())
            } else {
                part["text"].as_str().map(|text| {
                    if text.starts_with("[server tool result:") {
                        "server_tool_result".into()
                    } else if text.starts_with("[server tool:") {
                        "server_tool_use".into()
                    } else {
                        "text".into()
                    }
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn google_canonicalizes_search_declaration_and_history() {
        let request = super::google(
            &json!({
                "contents": [{"role": "model", "parts": [
                    {"text": "[server tool: google_search()]"},
                    {"text": "[server tool result: Fixture source]"},
                    {"text": "answer"}
                ]}],
                "tools": [{"googleSearch": {}}]
            }),
            "gemini-test",
        );
        assert_eq!(request.tool_names, ["WebSearch"]);
        assert_eq!(request.server_block_count, 2);
        assert_eq!(
            request.assistant_block_types,
            ["server_tool_use", "server_tool_result", "text",]
        );
    }
}
