//! Shared helpers for provider integration tests (wiremock SSE).

use futures::StreamExt;
use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, StreamChunk};

pub fn test_chat_params() -> ChatParams {
    ChatParams {
        model: "test-model".to_string(),
        messages: vec![Message::user("Hello")],
        system_prompt: "You are helpful.".to_string(),
        tools: vec![],
        max_tokens: 100,
        temperature: None,
        thinking: None,
        debug_dump_dir: None,
    }
}

pub async fn collect_chunks(
    mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<StreamChunk, LoopalError>> + Send + Unpin>,
    >,
) -> Vec<Result<StreamChunk, LoopalError>> {
    let mut chunks = Vec::new();
    while let Some(item) = stream.next().await {
        chunks.push(item);
    }
    chunks
}
