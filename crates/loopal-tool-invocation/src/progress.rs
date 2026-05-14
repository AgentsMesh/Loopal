use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    pub tail: String,
}

impl ProgressSnapshot {
    pub fn new(tail: impl Into<String>) -> Self {
        Self { tail: tail.into() }
    }
}
