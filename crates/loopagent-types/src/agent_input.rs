use crate::control::ControlCommand;
use crate::envelope::Envelope;

/// Input to the agent loop — either a data message or a control command.
///
/// Replaces the former `UserCommand` enum by preserving the full `Envelope`
/// (with source/target/id/timestamp) instead of flattening to a plain string.
/// Control commands pass through without adaptation.
#[derive(Debug, Clone)]
pub enum AgentInput {
    /// A data-plane message (human, agent, or channel).
    Message(Envelope),
    /// A control-plane command (mode switch, clear, compact, model switch).
    Control(ControlCommand),
}
