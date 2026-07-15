use serde_json::{Value, json};

pub fn message_start(chunks: &[Value]) -> Value {
    let usage = chunks.iter().find(|chunk| chunk["type"] == "usage");
    json!({
        "type": "message_start",
        "message": {"usage": {
            "input_tokens": usage.and_then(|value| value["input"].as_u64())
                .unwrap_or(if usage.is_some() { 10 } else { 0 }),
            "output_tokens": 0,
            "cache_creation_input_tokens": field(usage, "cache_creation"),
            "cache_read_input_tokens": field(usage, "cache_read")
        }}
    })
}

pub fn message_delta(chunk: Option<&Value>, stop_reason: Option<&str>) -> Value {
    json!({
        "type": "message_delta",
        "delta": stop_reason.map_or_else(|| json!({}), |reason| {
            json!({"stop_reason": reason})
        }),
        "usage": {
            "input_tokens": 0,
            "output_tokens": chunk.map_or(0, |value| {
                value["output"].as_u64().unwrap_or(5)
            }),
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0
        }
    })
}

fn field(usage: Option<&Value>, name: &str) -> u64 {
    usage.and_then(|value| value[name].as_u64()).unwrap_or(0)
}
