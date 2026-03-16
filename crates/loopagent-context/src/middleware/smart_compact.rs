use async_trait::async_trait;
use futures::StreamExt;
use loopagent_types::error::LoopAgentError;
use loopagent_types::message::{ContentBlock, Message, MessageRole};
use loopagent_types::middleware::{Middleware, MiddlewareContext};
use loopagent_types::provider::{ChatParams, StreamChunk};

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

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopAgentError> {
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
                conversation_text.push_str(&format!("{}: {}\n\n", role, content));
            }
            // Also summarize tool interactions
            for block in &msg.content {
                match block {
                    ContentBlock::ToolUse { name, .. } => {
                        conversation_text.push_str(&format!("[Tool call: {}]\n", name));
                    }
                    ContentBlock::ToolResult { content, is_error, .. } => {
                        let status = if *is_error { "error" } else { "ok" };
                        let preview = if content.len() > 200 {
                            format!("{}...[truncated]", &content[..200])
                        } else {
                            content.clone()
                        };
                        conversation_text.push_str(&format!("[Tool result ({}): {}]\n", status, preview));
                    }
                    _ => {}
                }
            }
        }

        // Ask LLM to summarize
        let summary_prompt = format!(
            "Summarize the following conversation concisely. \
             Focus on key decisions, findings, file changes, and important context. \
             Be brief but preserve critical information needed for continuing the conversation.\n\n\
             ---\n{}\n---\n\nProvide a concise summary:",
            conversation_text
        );

        let summary_params = ChatParams {
            model: ctx.model.clone(),
            messages: vec![Message::user(&summary_prompt)],
            system_prompt: "You are a conversation summarizer. Be concise and factual.".to_string(),
            tools: vec![],
            max_tokens: 1024,
            temperature: Some(0.0),
        };

        // Stream the summary response
        match provider.stream_chat(&summary_params).await {
            Ok(mut stream) => {
                let mut summary_text = String::new();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(StreamChunk::Text { text }) => summary_text.push_str(&text),
                        Ok(StreamChunk::Done) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "summarization stream error, falling back to truncation");
                            compact_messages(&mut ctx.messages, self.keep_last);
                            return Ok(());
                        }
                        _ => {}
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
            }
            Err(e) => {
                tracing::warn!(error = %e, "summarization request failed, falling back to truncation");
                compact_messages(&mut ctx.messages, self.keep_last);
            }
        }

        Ok(())
    }
}
