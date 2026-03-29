//! Translate message-related agent events to ACP SessionUpdate.

use agent_client_protocol_schema::{CurrentModeUpdate, SessionUpdate};

use crate::types::convert::text_chunk;

/// `Stream { text }` → `AgentMessageChunk`
pub fn translate_stream(text: &str) -> SessionUpdate {
    SessionUpdate::AgentMessageChunk(text_chunk(text))
}

/// `ThinkingStream { text }` → `AgentThoughtChunk`
pub fn translate_thinking(text: &str) -> SessionUpdate {
    SessionUpdate::AgentThoughtChunk(text_chunk(text))
}

/// `Error { message }` → `AgentMessageChunk` with `[error]` prefix
pub fn translate_error(message: &str) -> SessionUpdate {
    SessionUpdate::AgentMessageChunk(text_chunk(format!("[error] {message}")))
}

/// `ModeChanged { mode }` → `CurrentModeUpdate` (structured).
pub fn translate_mode_changed(mode: &str) -> SessionUpdate {
    SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(mode.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_produces_message_chunk() {
        let update = translate_stream("hello");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "agent_message_chunk");
        assert_eq!(val["content"]["text"], "hello");
    }

    #[test]
    fn thinking_produces_thought_chunk() {
        let update = translate_thinking("reasoning...");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "agent_thought_chunk");
        assert_eq!(val["content"]["text"], "reasoning...");
    }

    #[test]
    fn error_has_prefix() {
        let update = translate_error("connection lost");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["content"]["text"], "[error] connection lost");
    }

    #[test]
    fn mode_changed_is_structured() {
        let update = translate_mode_changed("plan");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "current_mode_update");
        assert_eq!(val["currentModeId"], "plan");
    }
}
