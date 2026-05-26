use loopal_error::Result;
use loopal_turn::{InjectionKind, TurnStep};
use tracing::warn;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Check tracked config files for changes; on first detection, append an
    /// `Injection { kind: ConfigRefresh, text: <reminder> }` step to the current
    /// turn so the LLM sees the updated context on its next call. Idempotent —
    /// `FileSnapshot::check_and_refresh` returns `None` when the file is
    /// unchanged.
    pub async fn check_and_inject_config_refresh(&mut self) -> Result<()> {
        let mut reminders = Vec::new();
        for snap in &mut self.config_snapshots {
            if let Some(r) = snap.check_and_refresh() {
                reminders.push(r);
            }
        }
        if reminders.is_empty() {
            return Ok(());
        }
        let text = format!(
            "<system-reminder>\n{}\n</system-reminder>",
            reminders.join("\n\n")
        );
        if let Err(e) = self.append_step_record(TurnStep::Injection {
            kind: InjectionKind::ConfigRefresh,
            text,
        }) {
            warn!(error = %e, "append_step(ConfigRefresh) failed");
        }
        Ok(())
    }
}
