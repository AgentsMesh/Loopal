use loopal_context::{
    ContextBudget, PersistError, TurnEventLogger, TurnStore, TurnStoreError, TurnTracker,
    TurnTrackerError,
};
use loopal_turn::{AssistantOutput, StopReason, TurnEvent, TurnOutcome, TurnStep, TurnTrigger};

struct InMemoryLogger;
impl TurnEventLogger for InMemoryLogger {
    fn persist(&self, _event: &TurnEvent) -> Result<(), PersistError> {
        Ok(())
    }
}

struct FailingLogger;
impl TurnEventLogger for FailingLogger {
    fn persist(&self, _event: &TurnEvent) -> Result<(), PersistError> {
        Err(PersistError("test failure".into()))
    }
}

fn budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 173_616,
        max_output_tokens: 64_000,
    }
}

fn user_trigger() -> TurnTrigger {
    TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: "hi".into(),
        images: Vec::new(),
    }
}

fn llm_step() -> TurnStep {
    TurnStep::LlmCall {
        model: "claude-opus-4-7".into(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![],
            stop_reason: StopReason::EndTurn,
        },
    }
}

fn tracker() -> TurnTracker {
    TurnTracker::new(TurnStore::new(), budget())
}

#[test]
fn empty_store_has_no_current_turn() {
    let t = tracker();
    assert!(t.store().current_turn().is_none());
    assert_eq!(t.store().len(), 0);
}

#[test]
fn start_turn_creates_in_progress_turn() {
    let mut t = tracker();
    let id = t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    assert_eq!(t.store().len(), 1);
    let cur = t.store().current_turn().unwrap();
    assert_eq!(cur.id, id);
    assert_eq!(cur.outcome, TurnOutcome::InProgress);
}

#[test]
fn append_step_requires_current_turn() {
    let mut t = tracker();
    let err = t.try_append_step(llm_step(), &InMemoryLogger).unwrap_err();
    assert!(matches!(err, TurnTrackerError::NoCurrentTurn));
}

#[test]
fn end_current_turn_clears_in_progress() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.try_append_step(llm_step(), &InMemoryLogger).unwrap();
    t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    assert!(t.store().current_turn().is_none());
    assert_eq!(t.store().turns()[0].outcome, TurnOutcome::Complete);
}

#[test]
fn cannot_append_after_end() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    let err = t.try_append_step(llm_step(), &InMemoryLogger).unwrap_err();
    assert!(matches!(err, TurnTrackerError::NoCurrentTurn));
}

#[test]
fn append_step_after_end_surfaces_as_no_current_turn() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    let err = t.try_append_step(llm_step(), &InMemoryLogger).unwrap_err();
    assert!(!matches!(
        err,
        TurnTrackerError::Store(TurnStoreError::CurrentTurnFinished)
    ));
}

#[test]
fn start_turn_rolls_back_on_persist_failure() {
    let mut t = tracker();
    let result = t.try_start_turn(user_trigger(), &FailingLogger);
    assert!(result.is_none());
    assert_eq!(t.store().len(), 0);
    assert!(t.store().current_turn().is_none());
}

#[test]
fn append_step_rolls_back_on_persist_failure() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    let result = t.try_append_step(llm_step(), &FailingLogger);
    assert!(matches!(result, Err(TurnTrackerError::PersistFailed(_))));
    assert_eq!(t.store().current_turn().unwrap().body.steps.len(), 0);
}

#[test]
fn end_turn_persist_failure_keeps_turn_in_progress() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    let result = t.end_turn(TurnOutcome::Complete, &FailingLogger);
    assert!(matches!(result, Err(TurnTrackerError::PersistFailed(_))));
    assert_eq!(t.store().turns()[0].outcome, TurnOutcome::InProgress);
    assert!(t.store().current_turn().is_some());
}

#[test]
fn from_turns_recovers_in_progress() {
    let mut t = tracker();
    let id = t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.try_append_step(llm_step(), &InMemoryLogger).unwrap();
    let turns = t.store().turns().to_vec();

    let restored = TurnStore::from_turns(turns);
    assert_eq!(restored.current_turn_id().unwrap(), &id);
}

#[test]
fn from_turns_no_current_when_all_complete() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    let turns = t.store().turns().to_vec();

    let restored = TurnStore::from_turns(turns);
    assert!(restored.current_turn_id().is_none());
}

#[test]
fn compaction_summary_drops_prior_turns_from_projected_view() {
    use loopal_turn::CompactionSummary;

    let mut t = tracker();
    t.try_start_turn(
        TurnTrigger::UserInput {
            envelope_id: "env-old-1".into(),
            content: "prior content one".into(),
            images: Vec::new(),
        },
        &InMemoryLogger,
    )
    .unwrap();
    t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();

    t.try_start_turn(
        TurnTrigger::UserInput {
            envelope_id: "env-old-2".into(),
            content: "prior content two".into(),
            images: Vec::new(),
        },
        &InMemoryLogger,
    )
    .unwrap();
    t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();

    t.try_start_turn(
        TurnTrigger::UserInput {
            envelope_id: "env-current".into(),
            content: "current user request".into(),
            images: Vec::new(),
        },
        &InMemoryLogger,
    )
    .unwrap();
    t.try_append_step(
        TurnStep::CompactionSummary(CompactionSummary {
            summary_text: "everything before is summarized".into(),
            ack_text: "ok".into(),
            kept_turn_count: 0,
            removed_turn_count: 2,
        }),
        &InMemoryLogger,
    )
    .unwrap();

    let flat: String = t
        .view()
        .messages()
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            loopal_provider_api::ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|");

    assert!(
        !flat.contains("prior content one") && !flat.contains("prior content two"),
        "prior turns must be dropped from projection: {flat}"
    );
    assert!(
        flat.contains("current user request"),
        "boundary turn trigger must survive: {flat}"
    );
    assert!(
        flat.contains("everything before is summarized"),
        "summary must appear: {flat}"
    );
}

#[test]
fn compact_boundary_turn_index_keeps_only_last_turn() {
    let mut t = tracker();
    for i in 0..5 {
        t.try_start_turn(
            TurnTrigger::UserInput {
                envelope_id: format!("env-{i}"),
                content: format!("msg-{i}"),
                images: Vec::new(),
            },
            &InMemoryLogger,
        )
        .unwrap();
        t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    }
    // 5 turns total; boundary should be 4 (keep last 1, summarize first 4).
    assert_eq!(t.store().compact_boundary_turn_index(), 4);
}

#[test]
fn compact_boundary_turn_index_zero_when_one_turn() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    // 1 turn: keep it, summarize nothing → boundary 0
    assert_eq!(t.store().compact_boundary_turn_index(), 0);
}

#[test]
fn compact_boundary_turn_index_zero_on_empty_store() {
    let t = tracker();
    assert_eq!(t.store().compact_boundary_turn_index(), 0);
}

#[test]
fn refresh_view_does_not_truncate_under_budget_pressure() {
    // Tiny budget: every byte counts. Pre-Phase-D refresh_view would have
    // truncated messages to fit; post-Phase-D it only projects + caps
    // individual oversized blocks. Wire path applies its own degradation.
    let tiny_budget = ContextBudget {
        context_window: 1_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 100,
        safety_margin: 50,
        message_budget: 200,
        max_output_tokens: 100,
    };
    let mut t = TurnTracker::new(TurnStore::new(), tiny_budget);
    for i in 0..10 {
        t.try_start_turn(
            TurnTrigger::UserInput {
                envelope_id: format!("e-{i}"),
                content: format!("very long content message number {i} with many tokens"),
                images: Vec::new(),
            },
            &InMemoryLogger,
        )
        .unwrap();
        t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    }
    // All 10 user messages survive in the projected view — refresh_view
    // doesn't apply enforce_budget anymore.
    let user_count = t
        .view()
        .messages()
        .iter()
        .filter(|m| m.role == loopal_provider_api::MessageRole::User)
        .count();
    assert!(
        user_count >= 10,
        "expected at least 10 user messages in view, got {user_count}"
    );
}

#[test]
fn update_budget_does_not_mutate_view_messages() {
    let mut t = tracker();
    for i in 0..3 {
        t.try_start_turn(
            TurnTrigger::UserInput {
                envelope_id: format!("e-{i}"),
                content: format!("message {i}"),
                images: Vec::new(),
            },
            &InMemoryLogger,
        )
        .unwrap();
        t.end_turn(TurnOutcome::Complete, &InMemoryLogger).unwrap();
    }
    let before = t.view().messages().len();
    // Drastically shrink budget — should NOT trigger immediate enforcement.
    let tiny = ContextBudget {
        context_window: 100,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 10,
        safety_margin: 5,
        message_budget: 50,
        max_output_tokens: 10,
    };
    t.update_budget(tiny);
    let after = t.view().messages().len();
    assert_eq!(
        before, after,
        "update_budget must not mutate view.messages directly"
    );
}

#[test]
fn with_wire_mut_commits_on_ok() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.try_append_step(llm_step(), &InMemoryLogger).unwrap();
    let initial_step_count = t.store().current_turn().unwrap().body.steps.len();
    let result: u32 = t.with_wire_mut(|turns| {
        // mutate first turn's step list: clear all steps (allowed on slice
        // via direct field access on Turn).
        if let Some(first) = turns.first_mut() {
            first.body.steps.clear();
        }
        42
    });
    assert_eq!(result, 42);
    // commit happened — the step we appended was cleared.
    let after_step_count = t.store().current_turn().unwrap().body.steps.len();
    assert_eq!(
        initial_step_count, 1,
        "precondition: 1 step before with_wire_mut"
    );
    assert_eq!(
        after_step_count, 0,
        "with_wire_mut must commit the clone (steps cleared) to the store"
    );
}

#[test]
#[should_panic(expected = "intentional panic in closure")]
fn with_wire_mut_does_not_commit_on_panic() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.try_append_step(llm_step(), &InMemoryLogger).unwrap();
    let initial_step_count = t.store().current_turn().unwrap().body.steps.len();
    // Use Arc to read the store after the panic-catch boundary.
    let snapshot = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let snapshot_clone = snapshot.clone();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.with_wire_mut(|turns| {
            // mutate then panic — clone should be dropped without commit.
            if let Some(first) = turns.first_mut() {
                first.body.steps.clear();
            }
            panic!("intentional panic in closure");
        });
    }));
    *snapshot_clone.lock().unwrap() = t.store().current_turn().unwrap().body.steps.len();
    // Store still has the original step count — clone was discarded.
    assert_eq!(
        *snapshot.lock().unwrap(),
        initial_step_count,
        "panic must roll back the cloned mutation"
    );
    panic!("intentional panic in closure");
}

#[test]
fn with_wire_mut_resets_tool_batch_step() {
    let mut t = tracker();
    t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    t.mark_tool_batch_open(0);
    assert_eq!(t.current_tool_batch_step(), Some(0));

    t.with_wire_mut(|_| ());

    // Defensive invariant: any wire mutation may invalidate the step index,
    // so it must be reset alongside clear/rewind/end_turn semantics.
    assert!(
        t.current_tool_batch_step().is_none(),
        "with_wire_mut must reset current_tool_batch_step"
    );
}

#[test]
fn try_start_turn_refuses_to_overwrite_in_progress() {
    // Regression for F7: previous try_start_turn called store.start_turn
    // unconditionally, silently orphaning the previous turn (still
    // InProgress in turns vec but current_turn_id moved to the new turn).
    let mut t = tracker();
    let first_id = t.try_start_turn(user_trigger(), &InMemoryLogger).unwrap();
    assert_eq!(t.store().len(), 1);

    // Attempt to open a second turn while the first is in progress.
    let second = t.try_start_turn(
        TurnTrigger::UserInput {
            envelope_id: "env-2".into(),
            content: "would orphan first".into(),
            images: Vec::new(),
        },
        &InMemoryLogger,
    );
    assert!(
        second.is_none(),
        "try_start_turn must return None when another turn is in progress"
    );
    assert_eq!(t.store().len(), 1, "store must remain at 1 turn");
    assert_eq!(
        t.store().current_turn_id(),
        Some(&first_id),
        "current_turn_id must still reference the first turn"
    );
}
