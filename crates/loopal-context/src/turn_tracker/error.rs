use super::logger::PersistError;
use crate::turn_store::TurnStoreError;

#[derive(Debug)]
pub enum TurnTrackerError {
    NoCurrentTurn,
    NoToolBatchOpen,
    Store(TurnStoreError),
    PersistFailed(PersistError),
}

impl std::fmt::Display for TurnTrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCurrentTurn => write!(f, "no turn in progress"),
            Self::NoToolBatchOpen => write!(f, "no in-flight ToolBatch step"),
            Self::Store(e) => write!(f, "turn store error: {e}"),
            Self::PersistFailed(e) => write!(f, "{e}; in-memory rolled back"),
        }
    }
}

impl std::error::Error for TurnTrackerError {}

impl From<TurnStoreError> for TurnTrackerError {
    fn from(e: TurnStoreError) -> Self {
        Self::Store(e)
    }
}
