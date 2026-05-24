use chrono::Utc;
use loopal_turn::{
    OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, TurnEvent, TurnId,
    TurnOutcome, TurnStep, TurnTrigger,
};
use tracing::warn;

use super::runner::AgentLoopRunner;

// reason: dual-write atomicity — turn_store (in-memory) 与 turns.jsonl (event-sourced)
// 必须同步成功或同步失败。任一失败时回滚另一方，避免 fold(jsonl) ≠ in-memory 的隐性
// 漂移；resume 路径靠 fold 还能恢复，前提是 jsonl 是真实记录。

impl AgentLoopRunner {
    fn persist_event(&self, event: &TurnEvent) -> bool {
        match self
            .params
            .deps
            .session_manager
            .record_turn_event(&self.params.session.id, event)
        {
            Ok(()) => true,
            Err(e) => {
                warn!(error = %e, "record_turn_event persist failed; rolling back in-memory update");
                false
            }
        }
    }

    pub(super) fn start_turn_record(&mut self, trigger: TurnTrigger) -> Option<TurnId> {
        let id = self.turns.store.start_turn(trigger.clone());
        let event = TurnEvent::TurnStarted {
            turn_id: id.clone(),
            started_at: Utc::now(),
            trigger,
        };
        if !self.persist_event(&event) {
            // Roll back: pop the just-pushed turn so turn_store stays in lockstep.
            self.turns.store.turns_mut().pop();
            return None;
        }
        self.turns.current_turn_id = Some(id.clone());
        self.turns.current_step_index = 0;
        self.turns.current_tool_batch_step = None;
        Some(id)
    }

    pub(super) fn append_step_record(&mut self, step: TurnStep) -> Option<u32> {
        let turn_id = self.turns.current_turn_id.as_ref()?.clone();
        let step_index = self.turns.current_step_index;
        // In-memory first so the event we persist matches what fold() would
        // reproduce. Failure here means the in-memory state machine is broken
        // (no current turn); skip the persist to avoid emitting an event that
        // can't be replayed.
        if let Err(e) = self.turns.store.append_step(step.clone()) {
            warn!(error = %e, "turn_store append_step failed; skipping event persist");
            return None;
        }
        let event = TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index,
            step,
        };
        if !self.persist_event(&event) {
            // Roll back the in-memory push so the two views stay aligned.
            if let Some(turn) = self
                .turns
                .store
                .turns_mut()
                .iter_mut()
                .find(|t| t.id == turn_id)
            {
                turn.body.steps.pop();
            }
            return None;
        }
        self.turns.current_step_index += 1;
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
        self.turns.current_tool_batch_step = Some(step_index);
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
        let Some(turn_id) = self.turns.current_turn_id.clone() else {
            return;
        };
        let Some(step_index) = self.turns.current_tool_batch_step else {
            warn!("update_tool_batch_item_state called without in-flight ToolBatch step");
            return;
        };
        // Snapshot the old state so we can roll back if event persist fails.
        let old_state = self
            .turns
            .store
            .current_turn()
            .and_then(|t| t.body.steps.get(step_index as usize))
            .and_then(|s| match s {
                TurnStep::ToolBatch(b) => b.items.get(item_index as usize).map(|i| i.state.clone()),
                _ => None,
            });
        if let Err(e) =
            self.turns
                .store
                .update_tool_state(step_index, item_index, new_state.clone())
        {
            warn!(error = %e, "turn_store update_tool_state failed; skipping event persist");
            return;
        }
        let event = TurnEvent::StepUpdated {
            turn_id,
            step_index,
            item_index,
            new_state,
        };
        if !self.persist_event(&event)
            && let Some(prev) = old_state
        {
            // Best-effort rollback to the prior state so in-memory matches
            // what fold(jsonl) will reproduce.
            let _ = self
                .turns
                .store
                .update_tool_state(step_index, item_index, prev);
        }
    }

    pub(super) fn close_tool_batch_record(&mut self) {
        self.turns.current_tool_batch_step = None;
    }

    pub(super) fn end_turn_record(&mut self, outcome: TurnOutcome) {
        let Some(turn_id) = self.turns.current_turn_id.clone() else {
            return;
        };
        if let Err(e) = self.turns.store.end_current_turn(outcome.clone()) {
            warn!(error = %e, "turn_store end_current_turn failed");
            return;
        }
        let event = TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome,
        };
        if !self.persist_event(&event) {
            // Rolling back end_current_turn requires re-opening — too invasive
            // for a log write failure on shutdown path. Leave in-memory ended;
            // resume from turns.jsonl will see InProgress and apply
            // CrashRecovery semantics, which converges to the same outcome.
            warn!(
                ?turn_id,
                "TurnEnded event persist failed; in-memory ended, jsonl missing"
            );
        }
        self.turns.current_turn_id = None;
        self.turns.current_step_index = 0;
        self.turns.current_tool_batch_step = None;
    }
}
