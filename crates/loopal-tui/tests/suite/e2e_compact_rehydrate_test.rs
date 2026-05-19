use loopal_context::compact_config::REHYDRATE_TOTAL_BYTES;
use loopal_context::middleware::touched_files::TouchedFile;
use loopal_message::{ContentBlock, MessageRole};
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, chunks};
use tokio_util::sync::CancellationToken;

async fn drain_events(
    rx: &mut tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
) -> Vec<AgentEventPayload> {
    tokio::task::yield_now().await;
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        out.push(ev.payload);
    }
    out
}

/// Drive `compact_rehydrate` through a real `Read` tool dispatch:
/// fixture-resident files must be read, results bundled into one
/// Assistant ToolUse message + one User ToolResult message, and the
/// final stats must report all attempts as succeeded.
#[tokio::test]
async fn rehydrate_reads_files_via_real_read_tool() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let a = h.fixture.create_file("a.txt", "content of A\n");
    let b = h.fixture.create_file("nested/b.txt", "content of B\n");

    let touched = vec![
        TouchedFile {
            path: a.to_string_lossy().into(),
            mutated: false,
            last_seen_msg_idx: 0,
        },
        TouchedFile {
            path: b.to_string_lossy().into(),
            mutated: true,
            last_seen_msg_idx: 1,
        },
    ];

    let mut runner = h.runner;
    runner.params.store.clear();
    let before = runner.params.store.len();

    let stats = runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 2);
    assert_eq!(stats.files_succeeded, 2);
    assert!(stats.bytes_injected >= "content of A\n".len() + "content of B\n".len());

    let msgs = runner.params.store.messages();
    assert_eq!(msgs.len(), before + 2);

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
    assert_eq!(tool_use_ids.len(), 2, "expected 2 Read tool_uses");

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
    assert!(
        bodies.iter().any(|b| b.contains("content of A")),
        "expected A body in tool_results, got: {bodies:?}",
    );
    assert!(
        bodies.iter().any(|b| b.contains("content of B")),
        "expected B body in tool_results, got: {bodies:?}",
    );
}

/// Empty touched-file list short-circuits — no tool calls, no messages.
#[tokio::test]
async fn rehydrate_noop_on_empty_touched() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let mut runner = h.runner;
    runner.params.store.clear();
    let before = runner.params.store.len();

    let stats = runner
        .compact_rehydrate(&[], &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 0);
    assert_eq!(stats.files_succeeded, 0);
    assert_eq!(runner.params.store.len(), before);
}

/// Non-existent files surface as failed reads — they must not produce
/// orphan ToolUse blocks (which would break pair invariants). All-fail
/// case is a true no-op on the store.
#[tokio::test]
async fn rehydrate_skips_unreadable_paths() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let touched = vec![
        TouchedFile {
            path: h
                .fixture
                .path()
                .join("does-not-exist.txt")
                .to_string_lossy()
                .into(),
            mutated: false,
            last_seen_msg_idx: 0,
        },
        TouchedFile {
            path: h
                .fixture
                .path()
                .join("also-missing.txt")
                .to_string_lossy()
                .into(),
            mutated: false,
            last_seen_msg_idx: 1,
        },
    ];

    let mut runner = h.runner;
    runner.params.store.clear();
    let before = runner.params.store.len();

    let stats = runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 2);
    assert_eq!(stats.files_succeeded, 0);
    assert_eq!(stats.bytes_injected, 0);
    assert_eq!(
        runner.params.store.len(),
        before,
        "no orphan ToolUse/ToolResult must be appended when every read fails"
    );
}

/// Mix of existing and missing files: succeeded count reports only the
/// real reads, and exactly that many ToolUse/ToolResult pairs land in
/// the store. Guards against either silent inflation (counting errors)
/// or pair imbalance (orphan ToolUse when ToolResult is dropped).
#[tokio::test]
async fn rehydrate_handles_partial_success() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let real = h.fixture.create_file("real.txt", "real body\n");
    let touched = vec![
        TouchedFile {
            path: real.to_string_lossy().into(),
            mutated: false,
            last_seen_msg_idx: 0,
        },
        TouchedFile {
            path: h.fixture.path().join("ghost.txt").to_string_lossy().into(),
            mutated: false,
            last_seen_msg_idx: 1,
        },
        TouchedFile {
            path: h
                .fixture
                .path()
                .join("phantom.txt")
                .to_string_lossy()
                .into(),
            mutated: true,
            last_seen_msg_idx: 2,
        },
    ];

    let mut runner = h.runner;
    runner.params.store.clear();
    let stats = runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 3);
    assert_eq!(stats.files_succeeded, 1);
    assert!(stats.bytes_injected >= "real body\n".len());

    let msgs = runner.params.store.messages();
    assert_eq!(msgs.len(), 2, "exactly one Assistant + one User msg");
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
    assert_eq!(
        tool_use_count, 1,
        "exactly one ToolUse for the surviving read",
    );
    assert_eq!(
        tool_result_count, 1,
        "exactly one ToolResult — pair invariant intact",
    );
}

/// `REHYDRATE_TOTAL_BYTES` (50K) caps cumulative injected bytes. With 6
/// files of 12K each, the cap bites well before all 6 read in (also
/// `REHYDRATE_TOP_N=5` limits attempts to 5).
#[tokio::test]
async fn rehydrate_respects_total_bytes_budget() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let body = "X".repeat(12_000);
    let mut touched = Vec::new();
    for i in 0..6 {
        let p = h.fixture.create_file(&format!("big-{i}.txt"), &body);
        touched.push(TouchedFile {
            path: p.to_string_lossy().into(),
            mutated: false,
            last_seen_msg_idx: i,
        });
    }

    let mut runner = h.runner;
    runner.params.store.clear();
    let stats = runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert!(
        stats.bytes_injected <= REHYDRATE_TOTAL_BYTES,
        "injected bytes ({}) must not exceed total cap ({REHYDRATE_TOTAL_BYTES})",
        stats.bytes_injected,
    );
    assert!(
        stats.files_succeeded >= 1,
        "at least one file must fit under the cap",
    );
}

/// Successful rehydrate emits a `[rehydrated N files, M bytes]` Stream
/// event so the frontend can render an inline status line.
#[tokio::test]
async fn rehydrate_emits_summary_stream_event() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let real = h.fixture.create_file("ok.txt", "body\n");
    let touched = vec![TouchedFile {
        path: real.to_string_lossy().into(),
        mutated: false,
        last_seen_msg_idx: 0,
    }];

    h.runner.params.store.clear();
    let stats = h
        .runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;
    assert_eq!(stats.files_succeeded, 1);

    let evts = drain_events(&mut h.event_rx).await;
    let stream_text = evts.iter().find_map(|e| match e {
        AgentEventPayload::Stream { text } if text.contains("rehydrated") => Some(text.clone()),
        _ => None,
    });
    let text = stream_text.expect("rehydrate Stream event must fire");
    assert!(
        text.contains("rehydrated 1 files"),
        "stream must report file count, got: {text:?}",
    );
    assert!(
        text.contains("bytes"),
        "stream must report byte count, got: {text:?}",
    );
}

/// Partial-failure path: model must see an explicit note in the user
/// message saying N files were skipped so it can re-Read them on demand
/// rather than assuming the rehydrate was exhaustive.
#[tokio::test]
async fn rehydrate_partial_failure_appends_model_visible_note() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let real = h.fixture.create_file("only.txt", "ok\n");
    let touched = vec![
        TouchedFile {
            path: real.to_string_lossy().into(),
            mutated: false,
            last_seen_msg_idx: 0,
        },
        TouchedFile {
            path: h
                .fixture
                .path()
                .join("missing-1.txt")
                .to_string_lossy()
                .into(),
            mutated: false,
            last_seen_msg_idx: 1,
        },
        TouchedFile {
            path: h
                .fixture
                .path()
                .join("missing-2.txt")
                .to_string_lossy()
                .into(),
            mutated: false,
            last_seen_msg_idx: 2,
        },
    ];

    let mut runner = h.runner;
    runner.params.store.clear();
    let stats = runner
        .compact_rehydrate(&touched, &CancellationToken::new())
        .await;

    assert_eq!(stats.files_attempted, 3);
    assert_eq!(stats.files_succeeded, 1);

    let msgs = runner.params.store.messages();
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
        "note must spell out skipped/attempted counts, got: {note:?}",
    );
}

/// Pre-cancelled token must short-circuit before any file read happens.
/// Crucially, the store must be untouched — no orphan ToolUse can be
/// persisted when rehydrate is aborted.
#[tokio::test]
async fn rehydrate_pre_cancelled_token_skips_persist() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let real = h.fixture.create_file("victim.txt", "should not be read\n");
    let touched = vec![TouchedFile {
        path: real.to_string_lossy().into(),
        mutated: false,
        last_seen_msg_idx: 0,
    }];

    let mut runner = h.runner;
    runner.params.store.clear();
    let before = runner.params.store.len();

    let cancel = CancellationToken::new();
    cancel.cancel();
    let stats = runner.compact_rehydrate(&touched, &cancel).await;

    assert!(stats.cancelled, "stats must record the cancellation");
    assert_eq!(stats.files_succeeded, 0);
    assert_eq!(stats.bytes_injected, 0);
    assert_eq!(
        runner.params.store.len(),
        before,
        "no message must be persisted when rehydrate is pre-cancelled",
    );
}

/// Even with several files queued, a cancel races the parallel reads
/// and must produce zero persisted messages — the select! drops the
/// in-flight Reads before any `save_message` runs.
#[tokio::test]
async fn rehydrate_cancel_during_reads_leaves_store_untouched() {
    let h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    let mut touched = Vec::new();
    for i in 0..5 {
        let p = h.fixture.create_file(&format!("f{i}.txt"), "body\n");
        touched.push(TouchedFile {
            path: p.to_string_lossy().into(),
            mutated: false,
            last_seen_msg_idx: i,
        });
    }

    let mut runner = h.runner;
    runner.params.store.clear();
    let before = runner.params.store.len();

    let cancel = CancellationToken::new();
    cancel.cancel();
    let stats = runner.compact_rehydrate(&touched, &cancel).await;

    assert!(stats.cancelled);
    assert_eq!(
        runner.params.store.len(),
        before,
        "store must remain pristine — no orphan ToolUse from aborted rehydrate",
    );
}
