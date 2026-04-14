//! Fork snapshot — update shared message snapshot before tool execution.

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Copy current conversation messages into the shared snapshot.
    ///
    /// Called before each tool execution batch so the Agent tool can
    /// read a consistent snapshot for building fork context.
    /// Only clones if this batch contains an Agent tool call.
    pub(super) fn update_fork_snapshot(
        &self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) {
        let has_agent_call = tool_uses.iter().any(|(_, name, _)| name == "Agent");
        if !has_agent_call {
            return;
        }
        if let Some(ref snapshot) = self.params.message_snapshot {
            match snapshot.write() {
                Ok(mut guard) => *guard = self.params.store.messages().to_vec(),
                Err(e) => {
                    tracing::warn!("fork snapshot write lock poisoned, recovering: {e}");
                    *e.into_inner() = self.params.store.messages().to_vec();
                }
            }
        }
    }
}
