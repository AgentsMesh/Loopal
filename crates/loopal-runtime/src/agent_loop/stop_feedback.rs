use loopal_turn::{InjectionKind, TurnStep};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn push_stop_feedback(&mut self, feedback: String) {
        if let Err(e) = self.append_step_record(TurnStep::Injection {
            kind: InjectionKind::StopFeedback,
            text: feedback,
        }) {
            error!(error = %e, "append_step(Injection::StopFeedback) failed");
        }
    }
}
