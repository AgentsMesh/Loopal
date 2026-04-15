use std::sync::Mutex;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider_api::{Middleware, MiddlewareContext};
use tracing::debug;

use super::file_snapshot::FileSnapshot;

pub struct ConfigRefreshMiddleware {
    snapshots: Mutex<Vec<FileSnapshot>>,
}

impl ConfigRefreshMiddleware {
    pub fn new(snapshots: Vec<FileSnapshot>) -> Self {
        Self {
            snapshots: Mutex::new(snapshots),
        }
    }
}

#[async_trait]
impl Middleware for ConfigRefreshMiddleware {
    fn name(&self) -> &str {
        "config_refresh"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        // Recover from poison — a panic in a previous holder shouldn't block future checks,
        // since the lock only protects file-snapshot metadata (mtime + content cache).
        let mut snapshots = self.snapshots.lock().unwrap_or_else(|e| e.into_inner());
        let mut reminders = Vec::new();

        for snap in snapshots.iter_mut() {
            if let Some(reminder) = snap.check_and_refresh() {
                debug!(label = snap.label(), "config file changed");
                reminders.push(reminder);
            }
        }
        drop(snapshots);

        if reminders.is_empty() {
            return Ok(());
        }

        let reminder_text = format!(
            "<system-reminder>\n{}\n</system-reminder>",
            reminders.join("\n\n")
        );
        debug!(count = reminders.len(), "injecting config refresh reminders");
        // User role (not System) — modifying the system prompt would invalidate
        // Anthropic's prefix cache. system-reminder XML tags are the established
        // convention for injecting context updates as user messages.
        ctx.messages.push(Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: reminder_text,
            }],
        });
        Ok(())
    }
}
