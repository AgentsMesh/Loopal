use loopal_context::ContextBudget;
use loopal_context::middleware::touched_files::rank_touched_files;
use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_provider_api::Message;
use loopal_test_support::tool_history::{ToolStep, tool_history_turn};
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_turn::{Turn, TurnOutcome, TurnTrigger};

fn tiny_budget() -> ContextBudget {
    ContextBudget {
        context_window: 500,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 50,
        safety_margin: 25,
        message_budget: 425,
        max_output_tokens: 50,
    }
}

fn padded_seed(n: usize) -> Vec<Message> {
    (0..n)
        .map(|i| Message::user(&format!("m{i}: {}", "x".repeat(100))))
        .collect()
}

fn phase_label(p: &CompactPhase) -> &'static str {
    match p {
        CompactPhase::Microcompact => "Microcompact",
        CompactPhase::Summarize => "Summarize",
        CompactPhase::Rehydrate => "Rehydrate",
        CompactPhase::Done => "Done",
    }
}

#[tokio::test]
async fn compact_emits_full_phase_sequence_with_rehydrate() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .messages(padded_seed(6))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    let touched_file = h.fixture.create_file("touched.txt", "rehydrated body\n");
    // Seed a Read tool history so the touched_files middleware records the
    // file and rehydrate picks it up.
    h.runner.seed_test_turns(vec![tool_history_turn(
        "go",
        vec![ToolStep::done_with_input(
            "Read",
            "t1",
            serde_json::json!({ "file_path": touched_file }),
            "rehydrated body",
        )],
    )]);
    // Manual compaction hosts its summary on the latest completed turn and
    // keeps that turn out of the summarized prefix. Add a tail so the Read
    // history above is part of the prefix whose touched files are rehydrated.
    let mut tail = Turn::new(TurnTrigger::UserInput {
        envelope_id: "tail".into(),
        content: "continue".into(),
        images: Vec::new(),
    });
    tail.outcome = TurnOutcome::Complete;
    h.runner.seed_test_turns(vec![tail]);
    let ranked = rank_touched_files(h.runner.turns.view().messages(), 5);
    assert!(
        ranked
            .iter()
            .any(|file| file.path == touched_file.to_string_lossy()),
        "seeded successful Read must be discoverable before compaction: {ranked:?}"
    );

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let positions: Vec<(usize, &str)> = evts
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            AgentEventPayload::CompactProgress { phase, .. } => Some((i, phase_label(phase))),
            AgentEventPayload::Compacted(_) => Some((i, "Compacted")),
            _ => None,
        })
        .collect();

    let find = |label: &str| positions.iter().find(|(_, p)| *p == label).map(|(i, _)| *i);

    let summarize = find("Summarize").expect("Summarize phase missing");
    let rehydrate = find("Rehydrate").expect("Rehydrate phase missing");
    let compacted = find("Compacted").expect("Compacted event missing");
    let done = find("Done").expect("Done phase missing");

    assert!(
        summarize < rehydrate && rehydrate < compacted,
        "Summarize, Rehydrate, and Compacted must be ordered: {positions:?}"
    );
    assert!(
        compacted < done,
        "Compacted must precede Done: {positions:?}"
    );
}

#[tokio::test]
async fn compact_skips_rehydrate_when_no_files_touched() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .messages(padded_seed(6))
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let saw_rehydrate = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::CompactProgress {
                phase: CompactPhase::Rehydrate,
                ..
            }
        )
    });
    let saw_done = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::CompactProgress {
                phase: CompactPhase::Done,
                ..
            }
        )
    });
    let saw_compacted = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Compacted(_)));

    assert!(!saw_rehydrate, "Rehydrate must be skipped: {evts:?}");
    assert!(saw_compacted, "Compacted still required: {evts:?}");
    assert!(saw_done, "Done still required: {evts:?}");
}
