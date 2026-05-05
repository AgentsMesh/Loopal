use serde::{Deserialize, Serialize};

use crate::state::SessionViewState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSnapshot {
    pub rev: u64,
    pub state: SessionViewState,
}
