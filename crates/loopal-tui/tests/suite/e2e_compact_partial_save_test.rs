#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;

use loopal_context::ContextBudget;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
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

fn padded(label: &str) -> Message {
    let body: String = (0..100u8).map(|i| char::from(b'a' + (i % 26))).collect();
    Message::user(&format!("{label}: {body}"))
}

#[tokio::test]
async fn force_compact_keeps_store_intact_when_save_message_fails() {
    // R2 risk in commit 30e97afc: the boundary marker must only fire
    // after both `save_message(summary)` and `save_message(ack)`
    // succeed. If save_message fails partway through, the in-memory
    // store must roll back via early return, and the persisted boundary
    // marker must NOT be written — otherwise replay would pick up a
    // marker pointing at a non-existent summary id.
    //
    // Force failure by stripping write permission from the session
    // directory before invoking compaction: the OpenOptions create+append
    // call inside `append_message` then returns EACCES.
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..30 {
        h.runner
            .params
            .store
            .push_user(padded(&format!("seed-{i}")));
    }
    let before_len = h.runner.params.store.len();
    let session_id = h.runner.params.session.id.clone();

    // wiring.rs builds `SessionManager::with_base_dir(fixture/sessions)`
    // and `MessageStore::messages_file` then nests `sessions/{id}/...`,
    // so the actual on-disk path is `fixture/sessions/sessions/{id}`.
    let session_dir = h
        .fixture
        .path()
        .join("sessions")
        .join("sessions")
        .join(&session_id);
    fs::create_dir_all(&session_dir).unwrap();
    let original_perm = fs::metadata(&session_dir).unwrap().permissions();
    let mut readonly = original_perm.clone();
    readonly.set_mode(0o500);
    fs::set_permissions(&session_dir, readonly).unwrap();

    let result = h.runner.force_compact(None).await;

    let _ = fs::set_permissions(&session_dir, original_perm);

    assert!(
        result.is_err(),
        "force_compact must propagate save_message failure, got: {result:?}",
    );
    assert_eq!(
        h.runner.params.store.len(),
        before_len,
        "save_message failure must leave the in-memory store untouched",
    );

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let saw_compacted = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Compacted(_)));
    assert!(
        !saw_compacted,
        "Compacted event must NOT fire when persistence failed: {evts:?}",
    );
}
