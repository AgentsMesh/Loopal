use loopal_provider_api::Message;
use loopal_runtime::hydrate::{hydrate_images, hydrate_turn_images, maybe_persist_inline_images};
use loopal_storage::FileResourceStore;
use loopal_tool_invocation::ToolImageBlock;
use loopal_turn::{OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, TurnStep};
use tempfile::tempdir;

use crate::hydrate_support::{MAX_IMAGE_BYTES, WriteFailingStore, b64, images, message, png};

#[tokio::test]
async fn small_valid_image_stays_inline() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let mut values = vec![ToolImageBlock::inline("image/png", b64(&png(1024)))];
    maybe_persist_inline_images(
        store.as_ref(),
        "sess",
        &mut values,
        256 * 1024,
        MAX_IMAGE_BYTES,
    )
    .await
    .unwrap();
    assert!(matches!(values[0], ToolImageBlock::Inline { .. }));
}

#[tokio::test]
async fn large_valid_image_persists_and_hydrates() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let payload = png(300 * 1024);
    let original = b64(&payload);
    let mut values = vec![ToolImageBlock::inline("image/png", original.clone())];
    maybe_persist_inline_images(
        store.as_ref(),
        "sess",
        &mut values,
        256 * 1024,
        MAX_IMAGE_BYTES,
    )
    .await
    .unwrap();
    let ToolImageBlock::SessionResource { byte_size, .. } = &values[0] else {
        panic!("expected session resource")
    };
    assert_eq!(*byte_size, payload.len());

    let mut messages = vec![message(values)];
    hydrate_images(&mut messages, store.as_ref(), "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap();
    let ToolImageBlock::Inline { data, media_type } = &images(&messages[0])[0] else {
        panic!("expected inline image")
    };
    assert_eq!(media_type, "image/png");
    assert_eq!(data, &original);
}

#[tokio::test]
async fn hydrate_leaves_valid_inline_untouched() {
    let original = b64(&png(64));
    let mut messages = vec![message(vec![ToolImageBlock::inline(
        "image/png",
        original.clone(),
    )])];
    let store = WriteFailingStore;
    hydrate_images(&mut messages, &store, "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap();
    let ToolImageBlock::Inline { data, .. } = &images(&messages[0])[0] else {
        panic!()
    };
    assert_eq!(data, &original);
}

#[tokio::test]
async fn write_failure_keeps_valid_inline_image() {
    let payload = png(300 * 1024);
    let mut values = vec![ToolImageBlock::inline("image/png", b64(&payload))];
    maybe_persist_inline_images(
        &WriteFailingStore,
        "sess",
        &mut values,
        256 * 1024,
        MAX_IMAGE_BYTES,
    )
    .await
    .unwrap();
    assert!(matches!(values[0], ToolImageBlock::Inline { .. }));
}

#[tokio::test]
async fn hydrate_skips_messages_without_tool_result() {
    let mut messages = vec![Message::user("just text"), Message::assistant("reply")];
    hydrate_images(&mut messages, &WriteFailingStore, "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap();
}

#[tokio::test]
async fn hydrate_turns_skip_pending_tool_items_without_reading_resources() {
    let mut turn = loopal_turn::Turn {
        id: loopal_turn::TurnId::from_string("pending-turn"),
        started_at: chrono::Utc::now(),
        trigger: loopal_turn::TurnTrigger::Resume,
        body: Default::default(),
        outcome: loopal_turn::TurnOutcome::InProgress,
        last_step_at: None,
    };
    turn.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: ToolCall {
                id: ToolCallId::new("pending-tool"),
                name: "Read".into(),
                input: serde_json::json!({}),
            },
            state: ToolExecState::Pending,
        }],
    }));
    let mut turns = vec![turn];

    hydrate_turn_images(&mut turns, &WriteFailingStore, "sess", MAX_IMAGE_BYTES)
        .await
        .unwrap();
    let TurnStep::ToolBatch(batch) = &turns[0].body.steps[0] else {
        panic!("expected tool batch")
    };
    assert!(matches!(batch.items[0].state, ToolExecState::Pending));
}
