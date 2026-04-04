//! Async hook store — manages background hook tasks and rewake signaling.
//!
//! When an async hook completes with `rewake: true`, it sends an Envelope
//! to the agent loop via `rewake_tx`, waking the idle agent. This mirrors
//! the scheduler's `ScheduledTrigger → Envelope → select!` pattern.

use std::sync::{Arc, Mutex};

use loopal_protocol::{Envelope, MessageSource, UserContent};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::executor::ExecutorFactory;
use crate::output::interpret_output;

/// Tracks in-flight async hook tasks. Analogous to `BackgroundTaskStore`.
pub struct AsyncHookStore {
    rewake_tx: mpsc::Sender<Envelope>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl AsyncHookStore {
    pub fn new(rewake_tx: mpsc::Sender<Envelope>) -> Self {
        Self {
            rewake_tx,
            tasks: Mutex::new(Vec::new()),
        }
    }

    /// Spawn an async hook task. The task runs in the background;
    /// if exit code 2 or `rewake: true`, it sends an Envelope to wake the agent.
    pub fn spawn(
        &self,
        config: &loopal_config::HookConfig,
        input: serde_json::Value,
        factory: &Arc<dyn ExecutorFactory>,
    ) {
        let Some(executor) = factory.create(config) else {
            return; // Misconfigured hook, already logged by factory.
        };
        let tx = self.rewake_tx.clone();
        let handle = tokio::spawn(async move {
            match executor.execute(input).await {
                Ok(raw) => {
                    let output = interpret_output(&raw);
                    if output.rewake {
                        let content = output.additional_context.unwrap_or_default();
                        let env = Envelope::new(
                            MessageSource::System("hook".into()),
                            "self",
                            UserContent::text_only(content),
                        );
                        let _ = tx.send(env).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "async hook failed");
                }
            }
        });
        if let Ok(mut tasks) = self.tasks.lock() {
            // Housekeeping: remove completed tasks to prevent unbounded growth.
            tasks.retain(|h| !h.is_finished());
            tasks.push(handle);
        }
    }

    /// Clean up completed tasks (housekeeping, not required for correctness).
    pub fn cleanup_completed(&self) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.retain(|h| !h.is_finished());
        }
    }
}
