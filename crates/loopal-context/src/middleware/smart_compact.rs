use async_trait::async_trait;
use futures::StreamExt;
use loopal_error::LoopalError;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider_api::{ChatParams, Middleware, MiddlewareContext, StreamChunk};

use crate::compaction::compact_messages;
use crate::token_counter::estimate_messages_tokens;

/// Smart context compaction that uses LLM summarization instead of
/// simply truncating old messages. Falls back to traditional compaction
/// if no summarization provider is available.
pub struct SmartCompact {
    pub keep_last: usize,
}

impl SmartCompact {
    pub fn new(keep_last: usize) -> Self {
        Self { keep_last }
    }
}

#[async_trait]
impl Middleware for SmartCompact {
    fn name(&self) -> &str {
        "smart_compact"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        let estimated = estimate_messages_tokens(&ctx.messages);

        if estimated <= ctx.max_context_tokens {
            return Ok(());
        }

        tracing::info!(
            estimated,
            max = ctx.max_context_tokens,
            messages = ctx.messages.len(),
            "context exceeds limit, attempting smart compaction"
        );

        // If no provider available, fall back to traditional compaction
        let Some(provider) = &ctx.summarization_provider else {
            tracing::info!("no summarization provider, falling back to truncation");
            compact_messages(&mut ctx.messages, self.keep_last);
            return Ok(());
        };

        // Split messages: messages to summarize vs. messages to keep
        if ctx.messages.len() <= self.keep_last {
            return Ok(());
        }
        let split_at = ctx.messages.len() - self.keep_last;
        let old_messages = &ctx.messages[..split_at];

        if old_messages.is_empty() {
            return Ok(());
        }

        // Build a summary prompt from old messages
        let mut conversation_text = String::new();
        for msg in old_messages {
            let role = match msg.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
            };
            let content = msg.text_content();
            if !content.is_empty() {
                conversation_text.push_str(&format!("{role}: {content}\n\n"));
            }
            // Also summarize tool interactions
            for block in &msg.content {
                match block {
                    ContentBlock::ToolUse { name, .. } => {
                        conversation_text.push_str(&format!("[Tool call: {name}]\n"));
                    }
                    ContentBlock::ToolResult {
                        content, is_error, ..
                    } => {
                        let status = if *is_error { "error" } else { "ok" };
                        let preview = if content.len() > 200 {
                            let mut end = 200;
                            while end > 0 && !content.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...[truncated]", &content[..end])
                        } else {
                            content.clone()
                        };
                        conversation_text
                            .push_str(&format!("[Tool result ({status}): {preview}]\n"));
                    }
                    _ => {}
                }
            }
        }

        // Ask LLM to summarize with coding-agent-specific preservation rules
        let summary_prompt = format!(
            "You are summarizing a coding agent's conversation for context compaction.\n\n\
             PRESERVE:\n\
             - The user's original request and current intent\n\
             - All file paths that were read, created, or modified\n\
             - Key decisions and their rationale\n\
             - Error messages encountered and how they were resolved\n\
             - Current task state: what is done, what remains\n\n\
             OMIT:\n\
             - Verbatim file contents (summarize what was found instead)\n\
             - Redundant tool call details (group similar operations)\n\n\
             Conversation:\n---\n{conversation_text}\n---\n\n\
             Provide a structured summary:",
        );

        let summary_params = ChatParams {
            model: ctx.compact_model.as_ref().unwrap_or(&ctx.model).clone(),
            messages: vec![Message::user(&summary_prompt)],
            system_prompt: "You are a conversation summarizer. Be concise and factual.".to_string(),
            tools: vec![],
            max_tokens: 1024,
            temperature: Some(0.0),
            thinking: None,
            debug_dump_dir: None,
        };

        // Stream the summary response
        match provider.stream_chat(&summary_params).await {
            Ok(mut stream) => {
                let mut summary_text = String::new();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(StreamChunk::Text { text }) => summary_text.push_str(&text),
                        Ok(StreamChunk::Done { .. }) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "summarization stream error, falling back to truncation");
                            compact_messages(&mut ctx.messages, self.keep_last);
                            return Ok(());
                        }
                        Ok(
                            StreamChunk::Thinking { .. }
                            | StreamChunk::ThinkingSignature { .. }
                            | StreamChunk::ToolUse { .. }
                            | StreamChunk::Usage { .. },
                        ) => {}
                    }
                }

                if summary_text.is_empty() {
                    tracing::warn!("empty summary response, falling back to truncation");
                    compact_messages(&mut ctx.messages, self.keep_last);
                    return Ok(());
                }

                tracing::info!(
                    summary_len = summary_text.len(),
                    old_messages = old_messages.len(),
                    "generated conversation summary"
                );

                // Replace old messages with a summary system message + kept messages
                let summary_msg = Message {
                    id: None,
                    role: MessageRole::User,
                    content: vec![ContentBlock::Text {
                        text: format!(
                            "[Conversation summary of {} earlier messages]\n\n{}",
                            old_messages.len(),
                            summary_text
                        ),
                    }],
                };

                let mut new_messages = vec![summary_msg];
                new_messages.extend_from_slice(&ctx.messages[split_at..]);
                ctx.messages = new_messages;

                // Post-compaction validation: if summary inflated tokens, fall back
                let post_tokens = estimate_messages_tokens(&ctx.messages);
                if post_tokens > ctx.max_context_tokens {
                    tracing::warn!(
                        post_tokens,
                        max = ctx.max_context_tokens,
                        "compaction inflated tokens, falling back to truncation"
                    );
                    compact_messages(&mut ctx.messages, self.keep_last);
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "summarization request failed, falling back to truncation");
                compact_messages(&mut ctx.messages, self.keep_last);
            }
        }

        Ok(())
    }
}
