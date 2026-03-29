//! Conversion helpers: construct ACP schema types from internal values.

use agent_client_protocol_schema::{
    AgentCapabilities, ContentBlock, ContentChunk, Implementation, InitializeResponse,
    NewSessionResponse, PromptResponse, ProtocolVersion, SessionId, SessionNotification,
    SessionUpdate, StopReason, TextContent,
};

/// Create a text ContentBlock.
pub fn text_content_block(text: impl Into<String>) -> ContentBlock {
    ContentBlock::Text(TextContent::new(text))
}

/// Create a ContentChunk wrapping a text block.
pub fn text_chunk(text: impl Into<String>) -> ContentChunk {
    ContentChunk::new(text_content_block(text))
}

/// Build an `InitializeResponse` for Loopal with full capabilities.
pub fn make_init_response() -> InitializeResponse {
    let caps = AgentCapabilities::new().load_session(false);

    InitializeResponse::new(ProtocolVersion::V1)
        .agent_capabilities(caps)
        .agent_info(Implementation::new("loopal", env!("CARGO_PKG_VERSION")))
}

/// Build a `NewSessionResponse`.
pub fn make_new_session_response(session_id: impl Into<SessionId>) -> NewSessionResponse {
    NewSessionResponse::new(session_id)
}

/// Build a `PromptResponse`.
pub fn make_prompt_response(stop: StopReason) -> PromptResponse {
    PromptResponse::new(stop)
}

/// Build a `SessionNotification` from a `SessionUpdate`.
pub fn make_session_notification(session_id: &str, update: SessionUpdate) -> serde_json::Value {
    let notif = SessionNotification::new(SessionId::new(session_id), update);
    serde_json::to_value(notif).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_content_block_creates_text() {
        let block = text_content_block("hello");
        let val = serde_json::to_value(&block).unwrap();
        assert_eq!(val["type"], "text");
        assert_eq!(val["text"], "hello");
    }

    #[test]
    fn text_chunk_wraps_content() {
        let chunk = text_chunk("world");
        let val = serde_json::to_value(&chunk).unwrap();
        assert_eq!(val["content"]["text"], "world");
    }

    #[test]
    fn init_response_serializes() {
        let resp = make_init_response();
        let val = serde_json::to_value(&resp).unwrap();
        assert_eq!(val["protocolVersion"], 1);
        assert_eq!(val["agentInfo"]["name"], "loopal");
    }

    #[test]
    fn new_session_response_has_session_id() {
        let resp = make_new_session_response("s-42");
        let val = serde_json::to_value(&resp).unwrap();
        assert_eq!(val["sessionId"], "s-42");
    }

    #[test]
    fn prompt_response_has_stop_reason() {
        let resp = make_prompt_response(StopReason::EndTurn);
        let val = serde_json::to_value(&resp).unwrap();
        assert_eq!(val["stopReason"], "end_turn");

        let resp2 = make_prompt_response(StopReason::MaxTurnRequests);
        let val2 = serde_json::to_value(&resp2).unwrap();
        assert_eq!(val2["stopReason"], "max_turn_requests");
    }

    #[test]
    fn session_notification_format() {
        let update = SessionUpdate::AgentMessageChunk(text_chunk("hello"));
        let val = make_session_notification("s1", update);
        assert_eq!(val["sessionId"], "s1");
        assert_eq!(val["update"]["sessionUpdate"], "agent_message_chunk");
        assert_eq!(val["update"]["content"]["text"], "hello");
    }
}
