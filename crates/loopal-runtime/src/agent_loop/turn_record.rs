use chrono::Utc;
use loopal_turn::{
    OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, TurnEvent, TurnId,
    TurnOutcome, TurnStep, TurnTrigger,
};
use tracing::warn;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn start_turn_record(&mut self, trigger: TurnTrigger) -> TurnId {
        let id = self.turn_store.start_turn(trigger.clone());
        self.current_turn_id = Some(id.clone());
        self.current_step_index = 0;
        self.current_tool_batch_step = None;
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
        if let Err(e) = self.turn_store.append_step(step.clone()) {
            warn!(error = %e, "turn_store append_step failed");
        }
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

    /// Open a ToolBatch step in Pending state, carrying the full ToolCall info
    /// (name + input). Returns the step_index used by subsequent
    /// `update_tool_batch_item_state` calls. Recorded in both in-memory
    /// turn_store and the event-sourced turn log.
    pub(super) fn start_tool_batch_record(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Option<u32> {
        if tool_uses.is_empty() {
            return None;
        }
        let items: Vec<ToolBatchItem> = tool_uses
            .iter()
            .map(|(id, name, input)| ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new(id),
                    name: name.clone(),
                    input: input.clone(),
                },
                state: ToolExecState::Pending,
            })
            .collect();
        let step_index =
            self.append_step_record(TurnStep::ToolBatch(OrderedToolBatch { items }))?;
        self.current_tool_batch_step = Some(step_index);
        Some(step_index)
    }

    /// Patch one tool item's state. Identifies the in-flight ToolBatch step via
    /// `current_tool_batch_step`. Used by tools_finalize to mark each item
    /// Done/Cancelled without re-emitting a new ToolBatch step.
    pub(super) fn update_tool_batch_item_state(
        &mut self,
        item_index: u32,
        new_state: ToolExecState,
    ) {
        let Some(turn_id) = self.current_turn_id.clone() else {
            return;
        };
        let Some(step_index) = self.current_tool_batch_step else {
            warn!("update_tool_batch_item_state called without in-flight ToolBatch step");
            return;
        };
        if let Err(e) = self
            .turn_store
            .update_tool_state(step_index, item_index, new_state.clone())
        {
            warn!(error = %e, "turn_store update_tool_state failed");
        }
        let event = TurnEvent::StepUpdated {
            turn_id,
            step_index,
            item_index,
            new_state,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .record_turn_event(&self.params.session.id, &event)
        {
            warn!(error = %e, "record_turn_event StepUpdated failed");
        }
    }

    pub(super) fn close_tool_batch_record(&mut self) {
        self.current_tool_batch_step = None;
    }

    pub(super) fn end_turn_record(&mut self, outcome: TurnOutcome) {
        let Some(turn_id) = self.current_turn_id.take() else {
            return;
        };
        if let Err(e) = self.turn_store.end_current_turn(outcome.clone()) {
            warn!(error = %e, "turn_store end_current_turn failed");
        }
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
        self.current_tool_batch_step = None;
    }
}
