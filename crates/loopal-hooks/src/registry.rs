use loopal_config::hook_condition::matches_condition;
use loopal_config::{HookConfig, HookEvent};

/// Registry holding hook configurations and matching logic.
pub struct HookRegistry {
    hooks: Vec<HookConfig>,
}

impl HookRegistry {
    pub fn new(hooks: Vec<HookConfig>) -> Self {
        Self { hooks }
    }

    /// Return hooks matching the given event, optional tool name, and optional input.
    ///
    /// Matching priority: `condition` field > `tool_filter` field > match all.
    /// Pass `tool_input` for condition expressions with globs (e.g. `"Bash(git push*)"`)
    /// to work correctly.
    pub fn match_hooks(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
        tool_input: Option<&serde_json::Value>,
    ) -> Vec<&HookConfig> {
        let null = serde_json::Value::Null;
        let input = tool_input.unwrap_or(&null);
        self.hooks
            .iter()
            .filter(|hook| {
                if hook.event != event {
                    return false;
                }
                // Priority 1: condition expression (new)
                if let Some(ref cond) = hook.condition {
                    return match tool_name {
                        Some(name) => matches_condition(cond, name, input),
                        None => cond == "*",
                    };
                }
                // Priority 2: legacy tool_filter
                if let Some(ref filters) = hook.tool_filter {
                    return match tool_name {
                        Some(name) => filters.iter().any(|f| f == name),
                        None => false,
                    };
                }
                // No filter: match all
                true
            })
            .collect()
    }
}
