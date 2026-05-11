#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DegradedAction {
    #[default]
    Fallback,
    Deny,
}
