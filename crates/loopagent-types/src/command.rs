/// Typed commands sent from the TUI to the agent loop via the input channel.
/// Replaces the previous approach of encoding control signals as magic strings
/// (e.g., "/__mode_switch:plan").
#[derive(Debug, Clone)]
pub enum UserCommand {
    /// A regular user message
    Message(String),
    /// Switch the agent's operating mode
    ModeSwitch(AgentMode),
    /// Clear all conversation history
    Clear,
    /// Compact old messages, keeping only the most recent
    Compact,
    /// Switch to a different model at runtime
    ModelSwitch(String),
}

/// Agent operating mode, mirrored here in types to avoid circular dependency
/// with loopagent-runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Act,
    Plan,
}
