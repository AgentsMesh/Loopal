//! Hook executor abstraction — the polymorphism point for execution strategies.
//!
//! New executor types (Command, Http, Prompt) implement `HookExecutor`
//! without modifying existing code (OCP). Callers depend on the trait,
//! not concrete types (DIP).

use loopal_error::HookError;

/// Raw output from any hook executor, before interpretation.
///
/// Maps directly to the exit-code protocol:
/// - exit 0: success
/// - exit 2: feedback injection / rewake
/// - other: non-blocking error
#[derive(Debug, Clone)]
pub struct RawHookOutput {
    /// Process exit code (or HTTP-status-derived equivalent).
    pub exit_code: i32,
    /// Primary output (stdout for command, body for HTTP, LLM text for prompt).
    pub stdout: String,
    /// Diagnostic output (stderr for command; empty for HTTP/prompt).
    pub stderr: String,
}

/// Trait for hook execution strategies.
///
/// Each implementation handles one transport (shell, HTTP, LLM prompt).
/// The `HookService` dispatches to the correct executor via `ExecutorFactory`.
#[async_trait::async_trait]
pub trait HookExecutor: Send + Sync {
    /// Execute the hook with the given JSON input payload.
    async fn execute(&self, input: serde_json::Value) -> Result<RawHookOutput, HookError>;
}

/// Factory for creating executors from hook configuration.
///
/// Implemented by Kernel (GRASP Creator: Kernel owns Provider for PromptExecutor).
pub trait ExecutorFactory: Send + Sync {
    /// Create a boxed executor for the given hook configuration.
    /// Returns None if the config is invalid (missing fields, unavailable provider).
    fn create(&self, config: &loopal_config::HookConfig) -> Option<Box<dyn HookExecutor>>;
}
