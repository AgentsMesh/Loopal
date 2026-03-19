use crate::command::AgentMode;

/// Control-plane commands that affect agent behaviour without carrying data.
///
/// Separated from data messages (`Envelope`) to enforce the Data/Control plane
/// boundary. Sent via a dedicated `control_tx` channel, never through the
/// `MessageRouter`.
///
/// Shutdown is signalled by dropping the `control_tx` sender — the receiver
/// in `UnifiedFrontend::recv_input()` returns `None`, terminating the loop.
#[derive(Debug, Clone)]
pub enum ControlCommand {
    /// Switch agent operating mode (Act / Plan).
    ModeSwitch(AgentMode),
    /// Clear all conversation history.
    Clear,
    /// Compact old messages, keeping only the most recent.
    Compact,
    /// Switch to a different model at runtime.
    ModelSwitch(String),
}
