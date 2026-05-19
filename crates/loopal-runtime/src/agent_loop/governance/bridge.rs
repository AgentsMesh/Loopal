use loopal_message::Message;

// Invariant: implementations must persist (save_message) and push to store
// atomically — on persist failure, skip the in-memory push so JSONL and
// store stay consistent. Half-writes break the closure guarantee that
// every `tool_use` has a visible `tool_result` next turn.
pub trait DataPlaneBridge {
    fn write_tool_result_stub(&mut self, msg: Message);
    fn push_system_note(&mut self, msg: Message);
}
