use loopal_context::compact_config::REHYDRATE_TOTAL_BYTES;
use loopal_context::middleware::touched_files::TouchedFile;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ContentBlock, MessageRole};
use loopal_test_support::tool_history::reopen_for_test;
use loopal_test_support::{HarnessBuilder, chunks};
use tokio_util::sync::CancellationToken;

fn touched_at(path: &std::path::Path, mutated: bool, idx: usize) -> TouchedFile {
    TouchedFile {
        path: path.to_string_lossy().into(),
        mutated,
        last_seen_msg_idx: idx,
    }
}

#[tokio::test]
async fn rehydrate_reads_files_via_real_read_tool() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let a = h.fixture.create_file("a.txt", "content of A\n");
    let b = h.fixture.create_file("nested/b.txt", "content of B\n");
    let touched = vec![touched_at(&a, false, 0), touched_at(&b, true, 1)];

    let before = h.runner.turns.view().len();
    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 2);
    assert_eq!(stats.files_succeeded, 2);
    assert!(stats.bytes_injected >= "content of A\n".len() + "content of B\n".len());

    let msgs = h.runner.turns.view().messages();
    assert_eq!(
        msgs.len(),
        before + 2,
        "+1 assistant ToolUse, +1 user ToolResult"
    );

    let assistant = msgs.iter().rev().nth(1).expect("assistant msg");
    assert_eq!(assistant.role, MessageRole::Assistant);
    let tool_use_ids: Vec<&str> = assistant
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, name, .. } if name == "Read" => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(tool_use_ids.len(), 2);

    let user = msgs.last().expect("user tool_results msg");
    assert_eq!(user.role, MessageRole::User);
    let bodies: Vec<&str> = user
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } if tool_use_ids.contains(&tool_use_id.as_str()) => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(bodies.len(), 2);
    assert!(bodies.iter().any(|b| b.contains("content of A")));
    assert!(bodies.iter().any(|b| b.contains("content of B")));
}

#[tokio::test]
async fn rehydrate_noop_on_empty_touched() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let before = h.runner.turns.view().len();
    let stats = h
        .runner
        .compact_rehydrate(&[], &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 0);
    assert_eq!(stats.files_succeeded, 0);
    assert_eq!(h.runner.turns.view().len(), before);
}

#[tokio::test]
async fn rehydrate_skips_unreadable_paths() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let touched = vec![
        touched_at(&h.fixture.path().join("does-not-exist.txt"), false, 0),
        touched_at(&h.fixture.path().join("also-missing.txt"), false, 1),
    ];

    let before = h.runner.turns.view().len();
    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 2);
    assert_eq!(stats.files_succeeded, 0);
    assert_eq!(stats.bytes_injected, 0);
    assert_eq!(
        h.runner.turns.view().len(),
        before,
        "no orphan ToolUse/ToolResult when every read fails"
    );
}

#[tokio::test]
async fn rehydrate_handles_partial_success() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let real = h.fixture.create_file("real.txt", "real body\n");
    let touched = vec![
        touched_at(&real, false, 0),
        touched_at(&h.fixture.path().join("ghost.txt"), false, 1),
        touched_at(&h.fixture.path().join("phantom.txt"), true, 2),
    ];

    let before = h.runner.turns.view().len();
    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 3);
    assert_eq!(stats.files_succeeded, 1);
    assert!(stats.bytes_injected >= "real body\n".len());

    let msgs = h.runner.turns.view().messages();
    assert_eq!(msgs.len(), before + 2, "1 Assistant + 1 User");
    let tool_use_count = msgs
        .iter()
        .flat_map(|m| m.content.iter())
        .filter(|b| matches!(b, ContentBlock::ToolUse { name, .. } if name == "Read"))
        .count();
    let tool_result_count = msgs
        .iter()
        .flat_map(|m| m.content.iter())
        .filter(|b| matches!(b, ContentBlock::ToolResult { .. }))
        .count();
    assert_eq!(tool_use_count, 1);
    assert_eq!(tool_result_count, 1, "pair invariant intact");
}

#[tokio::test]
async fn rehydrate_respects_total_bytes_budget() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let body = "X".repeat(12_000);
    let touched: Vec<TouchedFile> = (0..6)
        .map(|i| {
            let p = h.fixture.create_file(&format!("big-{i}.txt"), &body);
            touched_at(&p, false, i)
        })
        .collect();

    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert!(
        stats.bytes_injected <= REHYDRATE_TOTAL_BYTES,
        "injected {} > cap {REHYDRATE_TOTAL_BYTES}",
        stats.bytes_injected
    );
    assert!(stats.files_succeeded >= 1, "at least one file must fit");
}

#[tokio::test]
async fn rehydrate_emits_summary_stream_event() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let real = h.fixture.create_file("ok.txt", "body\n");
    let touched = vec![touched_at(&real, false, 0)];

    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;
    assert_eq!(stats.files_succeeded, 1);

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let stream_text = evts.iter().find_map(|e| match e {
        AgentEventPayload::Stream { text } if text.contains("rehydrated") => Some(text.clone()),
        _ => None,
    });
    let text = stream_text.expect("rehydrate Stream event must fire");
    assert!(text.contains("rehydrated 1 files"));
    assert!(text.contains("bytes"));
}

#[tokio::test]
async fn rehydrate_partial_failure_appends_model_visible_note() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let real = h.fixture.create_file("only.txt", "ok\n");
    let touched = vec![
        touched_at(&real, false, 0),
        touched_at(&h.fixture.path().join("missing-1.txt"), false, 1),
        touched_at(&h.fixture.path().join("missing-2.txt"), false, 2),
    ];

    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 3);
    assert_eq!(stats.files_succeeded, 1);

    let msgs = h.runner.turns.view().messages();
    let user = msgs.last().expect("user message");
    let note = user
        .content
        .iter()
        .find_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .expect("partial-failure note must be present");
    assert!(
        note.contains("rehydrate partial: 2 of 3"),
        "note must spell out skipped/attempted, got: {note:?}"
    );
}

#[tokio::test]
async fn rehydrate_pre_cancelled_token_skips_persist() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let real = h.fixture.create_file("victim.txt", "should not be read\n");
    let touched = vec![touched_at(&real, false, 0)];

    let before = h.runner.turns.view().len();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let stats = h.runner.compact_rehydrate(&touched, &cancel).await;

    assert!(stats.cancelled);
    assert_eq!(stats.files_succeeded, 0);
    assert_eq!(stats.bytes_injected, 0);
    assert_eq!(h.runner.turns.view().len(), before);
}

#[tokio::test]
async fn rehydrate_cancel_during_reads_leaves_store_untouched() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    reopen_for_test(&mut h.runner);

    let touched: Vec<TouchedFile> = (0..5)
        .map(|i| {
            let p = h.fixture.create_file(&format!("f{i}.txt"), "body\n");
            touched_at(&p, false, i)
        })
        .collect();

    let before = h.runner.turns.view().len();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let stats = h.runner.compact_rehydrate(&touched, &cancel).await;

    assert!(stats.cancelled);
    assert_eq!(
        h.runner.turns.view().len(),
        before,
        "store must remain pristine — no orphan ToolUse from aborted rehydrate"
    );
}
