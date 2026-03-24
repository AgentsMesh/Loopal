use loopal_message::{ContentBlock, Message, MessageRole};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Record the assistant response as a message in the conversation history.
    /// Writes to both persistent storage and in-memory store.
    /// Block order: thinking → server blocks → text → client tool_uses.
    pub fn record_assistant_message(
        &mut self,
        assistant_text: &str,
        tool_uses: &[(String, String, serde_json::Value)],
        thinking_text: &str,
        thinking_signature: Option<&str>,
        server_blocks: Vec<ContentBlock>,
    ) {
        let mut assistant_content: Vec<ContentBlock> = Vec::new();

        // Thinking block goes first (Anthropic API requires this order).
        // Skip if signature is missing — an unsigned thinking block (e.g. from
        // an interrupted stream) fails API validation on the next multi-turn call.
        if !thinking_text.is_empty() && thinking_signature.is_some() {
            assistant_content.push(ContentBlock::Thinking {
                thinking: thinking_text.to_string(),
                signature: thinking_signature.map(String::from),
            });
        }

        // Server-side tool blocks (e.g. web_search) in stream order.
        for block in server_blocks {
            assistant_content.push(block);
        }

        if !assistant_text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: assistant_text.to_string(),
            });
        }
        for (id, name, input) in tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            });
        }

        if !assistant_content.is_empty() {
            let mut assistant_msg = Message {
                id: None,
                role: MessageRole::Assistant,
                content: assistant_content,
            };
            if let Err(e) = self
                .params
                .session_manager
                .save_message(&self.params.session.id, &mut assistant_msg)
            {
                error!(error = %e, "failed to persist message");
            }
            self.params.store.push_assistant(assistant_msg);
        }
    }
}
