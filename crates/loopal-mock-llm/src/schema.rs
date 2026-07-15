use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};

const CALL_FIELDS: &[&str] = &[
    "expect",
    "status",
    "retryAfterMs",
    "delayMs",
    "headers",
    "body",
    "chunks",
    "rawSse",
    "disconnectAfterEvents",
    "closeBeforeHeaders",
];

pub(crate) fn validate_call(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .context("scenario call must be an object")?;
    reject_unknown(object, CALL_FIELDS, "scenario call")?;
    if let Some(count) = object.get("disconnectAfterEvents") {
        ensure!(
            count.as_u64().is_some_and(|value| value > 0),
            "disconnectAfterEvents must be a positive integer"
        );
    }
    let chunks = object.get("chunks").and_then(Value::as_array);
    let raw = object.get("rawSse").and_then(Value::as_array);
    ensure!(
        chunks.is_none_or(Vec::is_empty) || raw.is_none_or(Vec::is_empty),
        "chunks and rawSse cannot both be populated"
    );
    if let Some(chunks) = chunks {
        validate_chunks(chunks)?;
    }
    Ok(())
}

pub(crate) fn validate_chunks(chunks: &[Value]) -> Result<()> {
    for chunk in chunks {
        let object = chunk
            .as_object()
            .context("stream chunk must be an object")?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .context("stream chunk requires a string type")?;
        let allowed = match kind {
            "delay" => &["type", "ms"][..],
            "text" | "thinking" => &["type", "text"],
            "thinking_signature" => &["type", "signature"],
            "tool_use" => &["type", "id", "name", "input", "inputFragments"],
            "server_tool_use" => &["type", "id", "name", "input", "inputFragments"],
            "server_tool_result" => &["type", "block_type", "tool_use_id", "content"],
            "usage" => &[
                "type",
                "input",
                "output",
                "thinking",
                "cache_creation",
                "cache_read",
            ],
            "done" => &["type", "reason"],
            "invalid_sse" => &["type", "data"],
            "disconnect" => &["type"],
            other => bail!("unsupported mock chunk type: {other}"),
        };
        reject_unknown(object, allowed, "stream chunk")?;
        validate_chunk(kind, object)?;
    }
    Ok(())
}

fn validate_chunk(kind: &str, object: &Map<String, Value>) -> Result<()> {
    match kind {
        "delay" => require_u64(object, "ms"),
        "text" | "thinking" => require_string(object, "text"),
        "thinking_signature" => require_string(object, "signature"),
        "tool_use" | "server_tool_use" => {
            require_nonempty(object, "id")?;
            require_nonempty(object, "name")?;
            ensure!(
                object.get("input").is_some_and(Value::is_object),
                "input must be an object"
            );
            validate_fragments(object)?;
            Ok(())
        }
        "server_tool_result" => {
            require_nonempty(object, "block_type")?;
            require_nonempty(object, "tool_use_id")?;
            ensure!(
                object.contains_key("content"),
                "server tool result requires content"
            );
            Ok(())
        }
        "usage" => [
            "input",
            "output",
            "thinking",
            "cache_creation",
            "cache_read",
        ]
        .into_iter()
        .try_for_each(|field| optional_u64(object, field)),
        "done" => {
            if let Some(reason) = object.get("reason") {
                ensure!(
                    matches!(
                        reason.as_str(),
                        Some("end_turn" | "max_tokens" | "pause_turn")
                    ),
                    "invalid done reason"
                );
            }
            Ok(())
        }
        "invalid_sse" => require_string(object, "data"),
        "disconnect" => Ok(()),
        _ => unreachable!(),
    }
}

fn validate_fragments(object: &Map<String, Value>) -> Result<()> {
    let Some(fragments) = object.get("inputFragments") else {
        return Ok(());
    };
    let fragments = fragments
        .as_array()
        .context("inputFragments must be an array")?;
    ensure!(!fragments.is_empty(), "inputFragments cannot be empty");
    let encoded = fragments
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("inputFragments must contain strings")
        })
        .collect::<Result<String>>()?;
    let parsed: Value = serde_json::from_str(&encoded).context("inputFragments must form JSON")?;
    ensure!(
        object.get("input") == Some(&parsed),
        "inputFragments must encode input"
    );
    Ok(())
}

fn reject_unknown(object: &Map<String, Value>, allowed: &[&str], label: &str) -> Result<()> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        bail!("unknown {label} field: {field}");
    }
    Ok(())
}

fn require_nonempty(object: &Map<String, Value>, field: &str) -> Result<()> {
    ensure!(
        object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty()),
        "{field} must be a non-empty string"
    );
    Ok(())
}

fn require_string(object: &Map<String, Value>, field: &str) -> Result<()> {
    ensure!(
        object.get(field).is_some_and(Value::is_string),
        "{field} must be a string"
    );
    Ok(())
}

fn require_u64(object: &Map<String, Value>, field: &str) -> Result<()> {
    ensure!(
        object.get(field).and_then(Value::as_u64).is_some(),
        "{field} must be an integer"
    );
    Ok(())
}

fn optional_u64(object: &Map<String, Value>, field: &str) -> Result<()> {
    if let Some(value) = object.get(field) {
        ensure!(value.as_u64().is_some(), "{field} must be an integer");
    }
    Ok(())
}
