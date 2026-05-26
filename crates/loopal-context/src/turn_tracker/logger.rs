use loopal_turn::TurnEvent;

#[derive(Debug, thiserror::Error)]
#[error("turn event persist failed: {0}")]
pub struct PersistError(pub String);

pub trait TurnEventLogger {
    fn persist(&self, event: &TurnEvent) -> Result<(), PersistError>;
}
