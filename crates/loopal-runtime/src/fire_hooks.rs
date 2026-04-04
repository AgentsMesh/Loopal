//! Thin convenience wrapper for firing hooks from the runtime.
//!
//! Avoids repeating `kernel.hook_service().run_hooks(...)` + error
//! handling at every call site. Outputs are intentionally discarded
//! for events that are observation-only (SessionStart/End, PreCompact, etc.).

use loopal_config::HookEvent;
use loopal_hooks::HookContext;
use loopal_kernel::Kernel;

/// Fire all matching hooks for an event, discarding outputs.
///
/// Used for observation-only events (SessionStart, SessionEnd, PreCompact, etc.)
/// where the hooks cannot influence the control flow.
pub async fn fire_hooks(kernel: &Kernel, event: HookEvent, ctx: &HookContext<'_>) {
    let _ = kernel.hook_service().run_hooks(event, ctx).await;
}
