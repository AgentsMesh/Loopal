use std::time::{Duration, SystemTime};

use loopal_context::ContextBudget;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_test_support::{HarnessBuilder, chunks};

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

fn read_tool_use(path: &str, id: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: id.into(),
            name: "Read".into(),
            input: serde_json::json!({ "file_path": path }),
        }],
        origin: None,
    }
}

fn tool_result(id: &str, body: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: body.into(),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        }],
        origin: None,
    }
}

fn padded_user(label: &str) -> Message {
    Message::user(&format!("{label}: {}", "x".repeat(100)))
}

#[tokio::test]
async fn compact_emits_full_phase_sequence_with_rehydrate() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .build()
        .await;

    let touched_file = h.fixture.create_file("touched.txt", "rehydrated body\n");

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    h.runner.params.store.push_user(padded_user("seed"));
    h.runner
        .params
        .store
        .push_assistant(read_tool_use(&touched_file.to_string_lossy(), "t1"));
    h.runner
        .params
        .store
        .push_tool_results(tool_result("t1", "rehydrated body"));
    for i in 0..6 {
        h.runner
            .params
            .store
            .push_user(padded_user(&format!("m{i}")));
    }

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;

    let positions: Vec<(usize, &str)> = evts
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            AgentEventPayload::CompactProgress { phase, .. } => Some(match phase {
                CompactPhase::Microcompact => (i, "Microcompact"),
                CompactPhase::Summarize => (i, "Summarize"),
                CompactPhase::Rehydrate => (i, "Rehydrate"),
                CompactPhase::Done => (i, "Done"),
            }),
            AgentEventPayload::Compacted(_) => Some((i, "Compacted")),
            _ => None,
        })
        .collect();

    let summarize_pos = positions
        .iter()
        .find(|(_, p)| *p == "Summarize")
        .map(|(i, _)| *i)
        .expect("Summarize phase missing");
    let rehydrate_pos = positions
        .iter()
        .find(|(_, p)| *p == "Rehydrate")
        .map(|(i, _)| *i)
        .expect("Rehydrate phase missing");
    let compacted_pos = positions
        .iter()
        .find(|(_, p)| *p == "Compacted")
        .map(|(i, _)| *i)
        .expect("Compacted event missing");
    let done_pos = positions
        .iter()
        .find(|(_, p)| *p == "Done")
        .map(|(i, _)| *i)
        .expect("Done phase missing");

    assert!(
        summarize_pos < rehydrate_pos,
        "Summarize must precede Rehydrate ({summarize_pos} vs {rehydrate_pos}): {positions:?}"
    );
    assert!(
        rehydrate_pos < compacted_pos,
        "Rehydrate must precede Compacted ({rehydrate_pos} vs {compacted_pos}): {positions:?}"
    );
    assert!(
        compacted_pos < done_pos,
        "Compacted must precede Done ({compacted_pos} vs {done_pos}): {positions:?}"
    );
}

#[tokio::test]
async fn compact_skips_rehydrate_when_no_files_touched() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..6 {
        h.runner
            .params
            .store
            .push_user(padded_user(&format!("m{i}")));
    }

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

#[tokio::test]
async fn microcompact_emits_only_microcompact_phase() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);

    h.runner.params.store.clear();
    h.runner.params.store.push_assistant(Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "u1".into(),
            name: "Read".into(),
            input: serde_json::json!({}),
        }],
        origin: None,
    });
    h.runner
        .params
        .store
        .push_tool_results(tool_result("u1", "body"));

    let stale = SystemTime::now() - Duration::from_secs(120);
    h.runner.params.store.record_assistant_activity(stale);

    h.runner.check_and_microcompact().await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let phases: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::CompactProgress { phase, .. } => Some(match phase {
                CompactPhase::Microcompact => "Microcompact",
                CompactPhase::Summarize => "Summarize",
                CompactPhase::Rehydrate => "Rehydrate",
                CompactPhase::Done => "Done",
            }),
            _ => None,
        })
        .collect();
    assert_eq!(
        phases,
        vec!["Microcompact"],
        "microcompact must emit exactly one phase event"
    );
}
