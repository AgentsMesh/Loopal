use std::time::Duration;

use futures::StreamExt;
use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};

use crate::prompt;

pub const DEFAULT_CLASSIFIER_TIMEOUT: Duration = Duration::from_secs(180);

pub(crate) async fn call_classifier(
    provider: &dyn Provider,
    model: &str,
    user_prompt: &str,
    timeout: Duration,
) -> Result<String, LoopalError> {
    match tokio::time::timeout(timeout, call_classifier_inner(provider, model, user_prompt)).await {
        Ok(result) => result,
        Err(_) => Err(LoopalError::Other(format!(
            "classifier LLM call timed out ({}s)",
            timeout.as_secs()
        ))),
    }
}

async fn call_classifier_inner(
    provider: &dyn Provider,
    model: &str,
    user_prompt: &str,
) -> Result<String, LoopalError> {
    let params = ChatParams {
        model: model.to_string(),
        messages: vec![Message::user(user_prompt)],
        system_prompt: prompt::system_prompt().to_string(),
        tools: vec![],
        max_tokens: 256,
        temperature: Some(0.0),
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    };

    let mut stream = provider.stream_chat(&params).await?;
    let mut response = String::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(StreamChunk::Text { text }) => response.push_str(&text),
            Ok(StreamChunk::Done { .. }) => break,
            Err(e) => return Err(e),
            _ => {}
        }
    }

    Ok(response)
}

pub(crate) fn parse_response(raw: &str) -> Option<(bool, String)> {
    let json_str = raw
        .trim()
        .strip_prefix("```json")
        .or_else(|| raw.trim().strip_prefix("```"))
        .unwrap_or(raw.trim());
    let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let should_block = value.get("should_block")?.as_bool()?;
    let reason = value
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("(no reason)")
        .to_string();
    Some((should_block, reason))
}
