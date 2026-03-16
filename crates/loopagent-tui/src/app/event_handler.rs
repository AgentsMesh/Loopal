use loopagent_types::event::AgentEvent;

use super::{App, AppState, DisplayMessage, DisplayToolCall};

impl App {
    /// Process an incoming agent event and update state accordingly.
    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Stream { text } => {
                self.streaming_text.push_str(&text);
            }
            AgentEvent::ToolCall { id, name, input } => {
                // Flush streaming text as an assistant message
                self.flush_streaming();
                // Find the last assistant message or create one, and add the tool call
                if let Some(last) = self.messages.last_mut()
                    && last.role == "assistant" {
                        last.tool_calls.push(DisplayToolCall {
                            name: name.clone(),
                            status: "pending".to_string(),
                            summary: format!("{}({})", name, truncate_json(&input, 60)),
                        });
                        return;
                    }
                // No assistant message yet; create one
                self.messages.push(DisplayMessage {
                    role: "assistant".to_string(),
                    content: String::new(),
                    tool_calls: vec![DisplayToolCall {
                        name: name.clone(),
                        status: "pending".to_string(),
                        summary: format!("{}({})", name, truncate_json(&input, 60)),
                    }],
                });
                let _ = (id,); // suppress unused warning
            }
            AgentEvent::ToolResult {
                id: _,
                name,
                result,
                is_error,
            } => {
                let status = if is_error { "error" } else { "success" };
                // Update the matching pending tool call
                for msg in self.messages.iter_mut().rev() {
                    for tc in msg.tool_calls.iter_mut().rev() {
                        if tc.name == name && tc.status == "pending" {
                            tc.status = status.to_string();
                            tc.summary = truncate_str(&result, 80);
                            return;
                        }
                    }
                }
            }
            AgentEvent::ToolPermissionRequest { id, name, input } => {
                self.flush_streaming();
                self.state = AppState::ToolConfirm { id, name, input };
            }
            AgentEvent::Error { message } => {
                self.flush_streaming();
                self.messages.push(DisplayMessage {
                    role: "error".to_string(),
                    content: message,
                    tool_calls: Vec::new(),
                });
            }
            AgentEvent::AwaitingInput => {
                self.flush_streaming();
                self.turn_count += 1;
                self.agent_idle = true;
            }
            AgentEvent::MaxTurnsReached { turns } => {
                self.flush_streaming();
                self.messages.push(DisplayMessage {
                    role: "system".to_string(),
                    content: format!("Max turns reached ({})", turns),
                    tool_calls: Vec::new(),
                });
            }
            AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                context_window,
            } => {
                self.token_count = input_tokens + output_tokens;
                self.context_window = context_window;
            }
            AgentEvent::ModeChanged { mode } => {
                self.mode = mode;
            }
            AgentEvent::Started => {}
            AgentEvent::Finished => {
                self.flush_streaming();
            }
        }
    }

    /// Flush any buffered streaming text into a DisplayMessage.
    pub(crate) fn flush_streaming(&mut self) {
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            // Append to last assistant message or create new one
            if let Some(last) = self.messages.last_mut()
                && last.role == "assistant" && last.tool_calls.is_empty() {
                    last.content.push_str(&text);
                    return;
                }
            self.messages.push(DisplayMessage {
                role: "assistant".to_string(),
                content: text,
                tool_calls: Vec::new(),
            });
        }
    }
}

pub(crate) fn truncate_json(value: &serde_json::Value, max_len: usize) -> String {
    let s = value.to_string();
    truncate_str(&s, max_len)
}

pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find the last valid char boundary at or before max_len
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
