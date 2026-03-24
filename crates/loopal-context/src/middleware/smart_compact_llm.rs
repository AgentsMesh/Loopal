//! LLM call for summarization — split from smart_compact.rs for 200-line limit.

use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};

use futures::StreamExt;

/// Call the LLM to generate a working state summary.
pub(super) async fn call_summarization_llm(
    provider: &dyn Provider,
    model: &str,
    conversation_text: &str,
) -> Result<String, LoopalError> {
    let summary_prompt = format!(
        "You are compacting a coding agent's conversation history.\n\n\
         Write a WORKING STATE document. The agent will continue using ONLY this \
         document + the recent messages that follow. The original conversation will \
         not be available.\n\n\
         Sections:\n\
         ## Task\nThe user's original request. Quote their exact words.\n\
         ## Progress\nFiles created/modified, commands run, tests passed/failed.\n\
         ## Decisions\nKey choices and rationale.\n\
         ## Current State\nWhat the agent was working on, including partial work.\n\
         ## Next Steps\nWhat remains, in priority order.\n\
         ## Key References\nExact file paths, function names, error messages, URLs.\n\n\
         Rules: be factual, use bullet points, preserve identifiers verbatim, \
         do NOT include file contents, do NOT narrate — summarize outcomes.\n\n\
         Conversation:\n---\n{conversation_text}\n---",
    );

    let params = ChatParams {
        model: model.to_string(),
        messages: vec![Message::user(&summary_prompt)],
        system_prompt: "You produce structured working state summaries.".to_string(),
        tools: vec![],
        max_tokens: 2048,
        temperature: Some(0.0),
        thinking: None,
        debug_dump_dir: None,
    };

    let mut stream = provider.stream_chat(&params).await?;
    let mut summary = String::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(StreamChunk::Text { text }) => summary.push_str(&text),
            Ok(StreamChunk::Done { .. }) => break,
            Err(e) => return Err(e),
            _ => {}
        }
    }

    Ok(summary)
}
