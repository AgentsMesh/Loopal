/// Trait for tools to send memory observations to the Memory Observer.
///
/// Decouples the Memory tool from the agent crate — the tool only needs
/// this trait, not the concrete channel implementation.
pub trait MemoryChannel: Send + Sync {
    /// Best-effort send. Returns Err if the observer channel is full or closed.
    fn try_send(&self, observation: String) -> Result<(), String>;
}
