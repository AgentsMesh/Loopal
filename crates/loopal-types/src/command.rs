/// Agent operating mode, mirrored here in types to avoid circular dependency
/// with loopal-runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Act,
    Plan,
}
