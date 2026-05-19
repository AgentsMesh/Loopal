use std::collections::HashMap;

use crate::TaskType;

/// Routes task types to specific model IDs.
///
/// Resolution: task-specific override → default model. No hardcoded
/// per-task fallbacks — the user picks the summarization model in
/// `settings.json` (model_routing.summarization).
#[derive(Debug, Clone)]
pub struct ModelRouter {
    default_model: String,
    overrides: HashMap<TaskType, String>,
}

impl ModelRouter {
    pub fn new(default_model: String) -> Self {
        Self {
            default_model,
            overrides: HashMap::new(),
        }
    }

    pub fn from_parts(default_model: String, routing: HashMap<TaskType, String>) -> Self {
        Self {
            default_model,
            overrides: routing,
        }
    }

    pub fn resolve(&self, task: TaskType) -> &str {
        self.overrides
            .get(&task)
            .map(String::as_str)
            .unwrap_or(&self.default_model)
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    /// Update the default model (e.g., on runtime `/model` switch).
    /// Also clears any `TaskType::Default` override so the new default takes effect.
    pub fn set_default(&mut self, model: String) {
        self.overrides.remove(&crate::TaskType::Default);
        self.default_model = model;
    }
}
