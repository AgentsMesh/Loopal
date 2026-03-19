use loopal_types::hook::{HookConfig, HookEvent};

/// Registry holding hook configurations and matching logic.
pub struct HookRegistry {
    hooks: Vec<HookConfig>,
}

impl HookRegistry {
    pub fn new(hooks: Vec<HookConfig>) -> Self {
        Self { hooks }
    }

    /// Return hooks matching the given event and optional tool name.
    pub fn match_hooks(&self, event: HookEvent, tool_name: Option<&str>) -> Vec<&HookConfig> {
        self.hooks
            .iter()
            .filter(|hook| {
                if hook.event != event {
                    return false;
                }
                // If hook has a tool_filter, the tool_name must match one of them
                if let Some(ref filters) = hook.tool_filter {
                    match tool_name {
                        Some(name) => filters.iter().any(|f| f == name),
                        None => false,
                    }
                } else {
                    true
                }
            })
            .collect()
    }
}