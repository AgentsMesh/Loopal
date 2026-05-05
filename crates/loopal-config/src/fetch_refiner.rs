use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FetchRefinerConfig {
    pub enabled: bool,
    pub threshold_bytes: usize,
}

impl Default for FetchRefinerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_bytes: 8 * 1024,
        }
    }
}
