//! End-to-end integration tests for `compaction_run.rs::run_smart_compact`
//! and `force_compact` — the production compaction pipeline.
//!
//! These cover the Phase B / Phase 6 fixes for the turn-space semantics:
//! - `compact_boundary_turn_index` returns turn-count, not message-count
//! - `removed_turn_count = boundary_turn_at`, `kept_turn_count = turn_count - boundary_turn_at`
//! - `CompactionSummary` step lands on the boundary turn (last in store)
//! - The projection drops everything before the CompactionSummary

use loopal_protocol::AgentEventPayload;
use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_turn::TurnStep;

fn five_user_messages() -> Vec<Message> {
    vec![
        Message::user("turn 1 content"),
        Message::user("turn 2 content"),
        Message::user("turn 3 content"),
        Message::user("turn 4 content"),
        Message::user("turn 5 content"),
    ]
}

#[tokio::test]
async fn force_compact_persists_compaction_summary_with_correct_turn_counts() {
    // 5 seeded user turns → force_compact reopens the user's most recent
    // Complete turn → summarizes the older 4 → CompactionSummary lands on
    // the reopened turn. No synthetic Resume turn is created (reopen
    // strategy avoids that phantom turn and guarantees the user's last
    // verbatim trigger survives wire projection).
    let calls = vec![chunks::text_turn(
        "<summary>compact summary content</summary>",
    )];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(five_user_messages())
        .build()
        .await;

    let result = h.runner.force_compact(None).await;
    assert!(result.is_ok(), "force_compact returned Err: {result:?}");

    let turns = h.runner.turns.store().turns();
    // 5 seeded UserInput turns — no extra Resume turn created.
    assert_eq!(turns.len(), 5, "expected exactly 5 turns (no Resume host)");

    let last_turn = turns.last().unwrap();
    let summary_step = last_turn
        .body
        .steps
        .iter()
        .find_map(|s| match s {
            TurnStep::CompactionSummary(cs) => Some(cs),
            _ => None,
        })
        .expect("CompactionSummary step must land on the reopened last turn");

    // Reopen path: boundary = len - 1 = 4 → summarize turns[0..4],
    // append summary to turn idx 4 (the reopened last user turn).
    // removed_turn_count = boundary_turn_at = 4
    // kept_turn_count = turn_count - boundary = 5 - 4 = 1
    assert_eq!(
        summary_step.removed_turn_count, 4,
        "removed_turn_count must equal boundary_turn_at"
    );
    assert_eq!(
        summary_step.kept_turn_count, 1,
        "kept_turn_count must equal turn_count - boundary_turn_at"
    );
}

#[tokio::test]
async fn force_compact_drops_prior_turns_from_projection() {
    let calls = vec![chunks::text_turn(
        "<summary>compacted body of older history</summary>",
    )];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(five_user_messages())
        .build()
        .await;

    let _ = h.runner.force_compact(None).await;

    let messages = h.runner.turns.view().messages();
    let flat: String = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            loopal_provider_api::ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|");

    // turns 1-4 are summarized away — none should appear verbatim.
    for prior in &[
        "turn 1 content",
        "turn 2 content",
        "turn 3 content",
        "turn 4 content",
    ] {
        assert!(
            !flat.contains(prior),
            "prior turn {prior:?} must be dropped from projection; got: {flat}"
        );
    }
    // turn 5 (the user's most recent trigger) is preserved verbatim.
    assert!(
        flat.contains("turn 5 content"),
        "last user turn must survive verbatim in projection; got: {flat}"
    );
    // The summary body must appear (it landed on the reopened last turn).
    assert!(
        flat.contains("compacted body of older history"),
        "summary body must appear in projection; got: {flat}"
    );
}

#[tokio::test]
async fn force_compact_emits_compacted_event_with_kept_removed_stats() {
    let calls = vec![chunks::text_turn("<summary>summary</summary>")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(five_user_messages())
        .build()
        .await;

    let _ = h.runner.force_compact(None).await;

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let compacted = evts.iter().find_map(|e| match e {
        AgentEventPayload::Compacted(s) => Some(s.clone()),
        _ => None,
    });
    let summary = compacted.expect("force_compact must emit Compacted event");

    assert_eq!(summary.strategy, "manual");
    // before: 5 user messages; after: [summary, ack, last_user_trigger] = 3
    // → summarized message count = 2. Exact value catches off-by-one regression.
    assert_eq!(
        summary.summarized, 2,
        "Compacted.summarized must reflect message-count delta (5 before → 3 after)"
    );
    assert!(
        summary.tokens_before > 0,
        "Compacted.tokens_before must report pre-compact token count"
    );
}

#[tokio::test]
async fn force_compact_in_progress_only_turn_bails_with_done_event() {
    // Regression for F2: in-progress single turn → previous guard checked
    // message-count (passing the > 2 threshold with multi-step turns)
    // but compact_boundary_turn_index = 0, causing silent no-op + stuck
    // CompactProgress::Summarize banner. New guard checks compactable turn
    // count (excluding the in-progress turn) and bails AND emits Done.
    let calls = vec![chunks::text_turn("unused-fallback")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build()
        .await;

    // Manually open the "single in-progress turn" scenario by ingesting one
    // user message — sets up a UserInput turn that's still open at compact.
    h.runner
        .start_turn_record(loopal_turn::TurnTrigger::UserInput {
            envelope_id: "env-1".into(),
            content: "lone message".into(),
            images: Vec::new(),
        })
        .expect("start_turn_record should succeed for empty store");

    // Sanity: one in-progress turn, nothing complete to compact.
    assert_eq!(h.runner.turns.store().turns().len(), 1);
    assert!(h.runner.turns.current_turn_id().is_some());

    let result = h.runner.force_compact(None).await;
    assert!(result.is_ok());

    let turns = h.runner.turns.store().turns();
    let has_summary = turns.iter().any(|t| {
        t.body
            .steps
            .iter()
            .any(|s| matches!(s, TurnStep::CompactionSummary(_)))
    });
    assert!(
        !has_summary,
        "in-progress single turn must not get a summary"
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let saw_done = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::CompactProgress {
                phase: loopal_protocol::CompactPhase::Done,
                ..
            }
        )
    });
    assert!(
        saw_done,
        "force_compact bail path must emit CompactProgress::Done to clear stale banner"
    );
}

#[tokio::test]
async fn force_compact_idle_reopens_last_completed_turn() {
    // Seeded turns are all complete (no current). force_compact must
    // REOPEN the user's most recent Complete turn to host the
    // CompactionSummary step — NOT open a synthetic Resume turn. This
    // ensures `find_compaction_drop_index` keeps the user's latest
    // verbatim trigger in wire projection.
    let calls = vec![chunks::text_turn("<summary>x</summary>")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(five_user_messages())
        .build()
        .await;

    let turns_before = h.runner.turns.store().turns().len();
    assert!(h.runner.turns.current_turn_id().is_none());

    let _ = h.runner.force_compact(None).await;

    let turns_after = h.runner.turns.store().turns().len();
    assert_eq!(
        turns_after, turns_before,
        "force_compact on idle must reopen — NOT add a synthetic turn"
    );
    let last = h.runner.turns.store().turns().last().unwrap();
    assert!(
        matches!(last.trigger, loopal_turn::TurnTrigger::UserInput { .. }),
        "reopened turn must keep its original UserInput trigger (not Resume)"
    );
    assert!(
        matches!(last.outcome, loopal_turn::TurnOutcome::Complete),
        "reopened turn must be closed Complete after compaction"
    );
    // Verify the CompactionSummary step landed on the reopened turn.
    let has_summary = last
        .body
        .steps
        .iter()
        .any(|s| matches!(s, TurnStep::CompactionSummary(_)));
    assert!(
        has_summary,
        "CompactionSummary must be appended to the reopened last turn"
    );
}

#[tokio::test]
async fn double_force_compact_preserves_last_user_input() {
    // Regression: a SECOND consecutive /compact must NOT swallow the
    // user's last verbatim trigger. Before the AlreadySummarized guard,
    // the second pass found last turn already summarized → reopen None →
    // fallthrough to SyntheticOpened → summary lands on a Resume turn
    // with project_trigger=None → wire dropped "turn 5 content".
    let calls = vec![chunks::text_turn("<summary>first pass</summary>")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(five_user_messages())
        .build()
        .await;

    // First /compact: reopens last turn, appends summary.
    let first = h.runner.force_compact(None).await;
    assert!(matches!(first, Ok(true)), "first /compact must succeed");
    let turns_after_first = h.runner.turns.store().turns().len();

    // Second /compact (no new user input in between).
    let second = h.runner.force_compact(None).await;
    assert!(
        matches!(second, Ok(false)),
        "second /compact must bail with Ok(false), got {second:?}"
    );

    // No phantom turn added.
    let turns_after_second = h.runner.turns.store().turns().len();
    assert_eq!(
        turns_after_second, turns_after_first,
        "second /compact must NOT add a phantom Resume turn"
    );

    // The user's last verbatim trigger ("turn 5 content") must remain
    // present in projection.
    let messages = h.runner.turns.view().messages();
    let flat: String = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            loopal_provider_api::ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        flat.contains("turn 5 content"),
        "last user verbatim trigger must survive after double /compact; got: {flat}"
    );

    // AlreadySummarized path must emit Done banner with explanatory detail
    // so UI's Summarize indicator clears cleanly (not just turn-count == 0).
    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let saw_done_detail = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::CompactProgress {
                phase: loopal_protocol::CompactPhase::Done,
                detail: Some(d),
            } if d.contains("nothing new to compact")
        )
    });
    assert!(
        saw_done_detail,
        "AlreadySummarized bail must emit CompactProgress::Done with 'nothing new to compact' detail"
    );
}

#[tokio::test]
async fn compact_with_cancelled_tail_does_not_create_double_summary() {
    let calls = vec![chunks::text_turn("<summary>first pass</summary>")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(five_user_messages())
        .build()
        .await;

    let first = h.runner.force_compact(None).await;
    assert!(matches!(first, Ok(true)), "first /compact must succeed");
    let turns_after_first = h.runner.turns.store().turns().len();

    // Manually open + cancel a turn to simulate "user typed something then
    // Esc'd before completion" → outcome=Cancelled, no LlmCall/ToolBatch
    // in body.
    h.runner
        .start_turn_record(loopal_turn::TurnTrigger::UserInput {
            envelope_id: "cancelled-env".into(),
            content: "user typed then escaped".into(),
            images: Vec::new(),
        })
        .expect("start cancelled turn");
    h.runner
        .end_turn_record(loopal_turn::TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::UserInterrupt,
        });
    let turns_before_second = h.runner.turns.store().turns().len();
    assert_eq!(turns_before_second, turns_after_first + 1);

    // Second /compact should bail (AlreadySummarized) because no new
    // LlmCall/ToolBatch landed since the prior summary — Cancelled tail
    // has no body steps that count as new content.
    let second = h.runner.force_compact(None).await;
    assert!(
        matches!(second, Ok(false)),
        "second /compact must bail with Ok(false), got {second:?}"
    );

    // No phantom Resume turn added, no double-summary.
    let turns_after_second = h.runner.turns.store().turns().len();
    assert_eq!(
        turns_after_second, turns_before_second,
        "second /compact must NOT add a phantom Resume turn after Cancelled tail"
    );
    let summary_count: usize = h
        .runner
        .turns
        .store()
        .turns()
        .iter()
        .flat_map(|t| t.body.steps.iter())
        .filter(|s| matches!(s, TurnStep::CompactionSummary(_)))
        .count();
    assert_eq!(
        summary_count, 1,
        "must have exactly ONE CompactionSummary after Cancelled-tail re-compact"
    );
}

#[tokio::test]
async fn force_compact_single_complete_turn_bails_no_flicker() {
    // Regression: single Complete turn → reopen succeeded but boundary=0
    // produced silent Ok(false) + UI banner flicker. Tighten guard to
    // require ≥2 compactable turns so single-turn case bails cleanly.
    let calls = vec![chunks::text_turn("unused-fallback")];
    let mut h = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![loopal_provider_api::Message::user("only message")])
        .build()
        .await;

    let turns_before = h.runner.turns.store().turns().len();
    assert_eq!(turns_before, 1, "fixture must seed exactly 1 Complete turn");

    let result = h.runner.force_compact(None).await;
    assert!(
        matches!(result, Ok(false)),
        "single-turn /compact must bail with Ok(false) (no flicker)"
    );

    let turns_after = h.runner.turns.store().turns().len();
    assert_eq!(turns_after, turns_before, "no phantom Resume turn");
    let has_summary = h.runner.turns.store().turns().iter().any(|t| {
        t.body
            .steps
            .iter()
            .any(|s| matches!(s, TurnStep::CompactionSummary(_)))
    });
    assert!(!has_summary, "single-turn bail must NOT append any summary");
}
