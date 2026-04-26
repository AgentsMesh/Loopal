use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;

/// Input to the agent loop — either a data message or a control command.
///
/// Replaces the former `UserCommand` enum by preserving the full `Envelope`
/// (with source/target/id/timestamp) instead of flattening to a plain string.
/// Control commands pass through without adaptation.
///
/// `Envelope` carries qualified addresses (variable hub paths), so the
/// variant sizes diverge — but boxing on every input would add a heap
/// allocation in the hot dispatch path. Allow the size difference instead.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum AgentInput {
    /// A data-plane message (human, agent, or channel).
    Message(Envelope),
    /// A control-plane command (mode switch, clear, compact, model switch).
    Control(ControlCommand),
}
