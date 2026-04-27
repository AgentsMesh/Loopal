//! `SessionResumeHook` — extension point for session-scoped runtime state
//! that must follow the agent across `ResumeSession` control commands.
//!
//! When the agent's session changes mid-process (e.g. the TUI sends a
//! `ResumeSession` control command), [`AgentLoopRunner::handle_resume_session`]
//! invokes every registered hook so they can swap their own per-session
//! state — cron storage, task list, etc.
//!
//! This trait lives in `loopal-runtime` deliberately so the runtime can
//! call back into agent state without depending on `loopal-scheduler` or
//! `loopal-agent` directly. Concrete implementations (the cron and task
//! adapters) live in `loopal-agent`.
//!
//! Hooks fire after the message history has already been swapped, so
//! they may rely on `AgentLoopRunner.params.session.id` reflecting the
//! new session.
//!
//! ## Failure semantics
//!
//! Hooks return `Result<(), SessionResumeError>`. Failure does not
//! abort the resume — the message-history swap is already committed
//! and bouncing here would leave the user without any visible session
//! change. Instead, the runtime collects every hook's failure into a
//! `SessionResumeWarnings` event so the frontend can surface them.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;

use async_trait::async_trait;

/// Non-fatal error from a [`SessionResumeHook`].
///
/// `hook` carries the human-readable name of the adapter that failed —
/// `Cow<'static, str>` so static names like `"cron"` or `"task"` don't
/// allocate, while a future plugin / dynamically-registered hook can
/// still own its label without an extra trait method. `reason` is the
/// failure description; both are forwarded to the frontend through
/// `SessionResumeWarnings` for user-facing diagnostics.
#[derive(Debug)]
pub struct SessionResumeError {
    pub hook: Cow<'static, str>,
    pub reason: String,
}

impl SessionResumeError {
    pub fn new(hook: impl Into<Cow<'static, str>>, reason: impl Into<String>) -> Self {
        Self {
            hook: hook.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for SessionResumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "session-resume hook `{}` failed: {}",
            self.hook, self.reason
        )
    }
}

impl Error for SessionResumeError {}

/// Notified when the agent runtime hot-swaps the active session.
///
/// Implementations should be cheap and best-effort: a failure here must
/// not abort the resume — the message history has already been replaced
/// at the time hooks are invoked. Returned errors are aggregated by the
/// runtime into a single `SessionResumeWarnings` event.
#[async_trait]
pub trait SessionResumeHook: Send + Sync {
    /// Called once per registered hook after a session swap completes.
    /// `new_session_id` is the id of the newly active session.
    async fn on_session_changed(&self, new_session_id: &str) -> Result<(), SessionResumeError>;
}
