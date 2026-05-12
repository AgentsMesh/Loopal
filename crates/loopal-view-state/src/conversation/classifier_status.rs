use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClassifierStatus {
    #[default]
    None,
    Running {
        elapsed_ms: u64,
    },
    Failed {
        reason: String,
    },
    Completed {
        answers: Vec<String>,
    },
}

impl ClassifierStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}
