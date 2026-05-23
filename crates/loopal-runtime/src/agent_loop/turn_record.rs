use chrono::Utc;
use loopal_turn::{TurnEvent, TurnId, TurnOutcome, TurnStep, TurnTrigger};
use tracing::warn;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn start_turn_record(&mut self, trigger: TurnTrigger) -> TurnId {
        let id = TurnId::new();
        self.current_turn_id = Some(id.clone());
        self.current_step_index = 0;
        let event = TurnEvent::TurnStarted {
            turn_id: id.clone(),
            started_at: Utc::now(),
            trigger,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .record_turn_event(&self.params.session.id, &event)
        {
            warn!(error = %e, "record_turn_event TurnStarted failed");
        }
        id
    }

    pub(super) fn append_step_record(&mut self, step: TurnStep) -> Option<u32> {
        let turn_id = self.current_turn_id.as_ref()?.clone();
        let step_index = self.current_step_index;
        self.current_step_index += 1;
        let event = TurnEvent::StepAppended {
            turn_id,
            step_index,
            step,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .record_turn_event(&self.params.session.id, &event)
        {
            warn!(error = %e, "record_turn_event StepAppended failed");
        }
        Some(step_index)
    }

    pub(super) fn end_turn_record(&mut self, outcome: TurnOutcome) {
        let Some(turn_id) = self.current_turn_id.take() else {
            return;
        };
        let event = TurnEvent::TurnEnded { turn_id, outcome };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .record_turn_event(&self.params.session.id, &event)
        {
            warn!(error = %e, "record_turn_event TurnEnded failed");
        }
        self.current_step_index = 0;
    }
}
