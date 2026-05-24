use loopal_context::TurnStore;
use loopal_turn::TurnId;

/// Domain-layer turn tracking state. Owned by `AgentLoopRunner` and mutated
/// only by the `turn_record` helpers — every mutation is fail-closed
/// (in-memory and event-sourced views stay in lockstep).
pub struct TurnTracker {
    /// `Some` between `start_turn_record` and `end_turn_record`.
    pub current_turn_id: Option<TurnId>,
    /// Next `step_index` to emit via `TurnEvent::StepAppended`.
    pub current_step_index: u32,
    /// Index of the in-flight `TurnStep::ToolBatch` step, if any. Set by
    /// `start_tool_batch_record`; consumed by `update_tool_batch_item_state`
    /// and cleared by `close_tool_batch_record`.
    pub current_tool_batch_step: Option<u32>,
    /// In-memory `Vec<Turn>` mirror of turns.jsonl. Kept in lockstep with the
    /// event-sourced log via fail-closed semantics in `turn_record`.
    pub store: TurnStore,
}

impl TurnTracker {
    pub fn new(store: TurnStore) -> Self {
        Self {
            current_turn_id: None,
            current_step_index: 0,
            current_tool_batch_step: None,
            store,
        }
    }

    /// Reset step-level state. Called after a turn ends or starts.
    pub fn reset_step_state(&mut self) {
        self.current_step_index = 0;
        self.current_tool_batch_step = None;
    }
}
