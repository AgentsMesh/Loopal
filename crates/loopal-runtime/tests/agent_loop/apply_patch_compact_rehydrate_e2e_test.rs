use loopal_provider_api::ContentBlock;
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_turn::{Turn, TurnOutcome, TurnStep, TurnTrigger};

#[tokio::test]
async fn real_apply_patch_history_is_rehydrated_after_compaction() {
    let patch =
        "*** Begin Patch\n*** Add File: patched.txt\n+content from apply patch\n*** End Patch";
    let mut h = HarnessBuilder::new()
        .calls(vec![
            chunks::tool_turn(
                "apply-1",
                "ApplyPatch",
                serde_json::json!({ "patch": patch }),
            ),
            chunks::text_turn("patch complete"),
            chunks::text_turn("<summary>patched the requested file</summary>"),
        ])
        .build()
        .await;

    // The mock provider drives the normal LLM -> real built-in tool -> LLM
    // pipeline. This is deliberately not a pre-seeded synthetic ToolResult.
    h.runner.run().await.expect("agent turn should complete");
    let patched = h.fixture.path().join("patched.txt");
    assert_eq!(
        std::fs::read_to_string(&patched).expect("ApplyPatch must create the file"),
        "content from apply patch\n"
    );

    // Compaction keeps the last turn. Append a tail turn so the real
    // ApplyPatch turn is in the summarized prefix and must be rediscovered via
    // its persisted ToolResult metadata.
    let mut tail = Turn::new(TurnTrigger::UserInput {
        envelope_id: "tail".into(),
        content: "continue".into(),
        images: Vec::new(),
    });
    tail.outcome = TurnOutcome::Complete;
    h.runner.seed_test_turns(vec![tail]);

    assert!(
        h.runner
            .force_compact(None)
            .await
            .expect("manual compaction should succeed"),
        "history should be compactable"
    );

    let rehydrated = h
        .runner
        .turns
        .store()
        .turns()
        .iter()
        .flat_map(|turn| turn.body.steps.iter())
        .find_map(|step| match step {
            TurnStep::CompactionRehydrate(rehydrate) => rehydrate
                .files
                .iter()
                .find(|file| file.path.ends_with("patched.txt")),
            _ => None,
        })
        .expect("compaction must rehydrate the file modified by ApplyPatch");
    assert!(rehydrated.content.contains("content from apply patch"));

    let recorded_calls = h.recorded_messages.lock().unwrap();
    assert_eq!(
        recorded_calls.len(),
        3,
        "two main-model calls plus one compaction-model call must use the mock provider"
    );
    assert!(recorded_calls[1].iter().any(|turn| {
        turn.body.steps.iter().any(|step| match step {
            TurnStep::ToolBatch(batch) => batch.items.iter().any(|item| {
                item.call.name == "ApplyPatch"
                    && matches!(
                        &item.state,
                        loopal_turn::ToolExecState::Done(result)
                            if !result.is_error
                                && matches!(
                                    result.metadata.as_ref(),
                                    Some(loopal_tool_invocation::ToolResultMetadata::ModifiedFiles {
                                        paths
                                    }) if paths.iter().any(|path| path.ends_with("patched.txt"))
                                )
                    )
            }),
            _ => false,
        })
    }));

    // The projected post-compaction history must also contain the paired Read
    // result that will be sent to the next real model request.
    assert!(h.runner.turns.view().messages().iter().any(|message| {
        message.content.iter().any(|block| match block {
            ContentBlock::ToolResult { content, .. } => {
                content.contains("content from apply patch")
            }
            _ => false,
        })
    }));
}
