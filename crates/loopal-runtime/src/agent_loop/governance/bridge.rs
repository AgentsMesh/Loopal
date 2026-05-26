use loopal_provider_api::Message;

/// Text-only injection bridge used by governance to surface `GovernanceFeedback`
/// / `StopFeedback` / `SystemNote` messages back to the LLM. The implementation
/// extracts `msg.text_content()` and writes a `TurnStep::Injection` with the
/// appropriate kind — only Text content is preserved. Pair-of-blocks payloads
/// (e.g. abort compensation `ToolResult`s) need to go through a different
/// path (`start_tool_batch_record` + `update_tool_batch_item_state`).
pub trait DataPlaneBridge {
    fn push_system_note(&mut self, msg: Message);
}
