//! Hook service — single entry point for hook orchestration.
//!
//! SRP: coordinates matching, execution, and output interpretation.
//! Consumers call `run_hooks()` and get back typed `HookOutput`s.

use std::sync::Arc;

use loopal_config::HookEvent;
use tracing::warn;

use crate::executor::ExecutorFactory;
use crate::input::{HookContext, build_hook_input};
use crate::output::{HookOutput, interpret_output, interpret_pre_tool_output};
use crate::registry::HookRegistry;

/// Central hook orchestration service.
///
/// Replaces the previous pattern of "registry.match + for loop + run_hook"
/// scattered across call sites. Now there's one entry point.
pub struct HookService {
    registry: HookRegistry,
    factory: Arc<dyn ExecutorFactory>,
}

impl HookService {
    pub fn new(registry: HookRegistry, factory: Arc<dyn ExecutorFactory>) -> Self {
        Self { registry, factory }
    }

    /// Run all matching hooks for an event. Returns aggregated outputs.
    ///
    /// Async hooks will be handled in a future phase (AsyncHookStore).
    /// Currently all hooks execute synchronously in sequence.
    pub async fn run_hooks(&self, event: HookEvent, context: &HookContext<'_>) -> Vec<HookOutput> {
        let matched = self
            .registry
            .match_hooks(event, context.tool_name, context.tool_input);
        if matched.is_empty() {
            return Vec::new();
        }

        let input = build_hook_input(event, context);
        let mut outputs = Vec::new();

        for config in matched {
            let Some(executor) = self.factory.create(config) else {
                continue; // Misconfigured hook, already logged by factory.
            };
            match executor.execute(input.clone()).await {
                Ok(raw) => {
                    // PreToolUse: non-zero exit = deny (backward compat)
                    let out = if event == HookEvent::PreToolUse {
                        interpret_pre_tool_output(&raw)
                    } else {
                        interpret_output(&raw)
                    };
                    outputs.push(out);
                }
                Err(e) => {
                    warn!(event = ?event, error = %e, "hook execution failed");
                }
            }
        }
        outputs
    }

    /// Access the underlying registry (for backward-compat call sites
    /// that still need direct matching).
    pub fn registry(&self) -> &HookRegistry {
        &self.registry
    }
}
