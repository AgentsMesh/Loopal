use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_tool_invocation::ToolResultMetadata;
use loopal_turn::{ToolExecState, TurnStep};

#[tokio::test]
async fn partial_apply_patch_reports_only_operations_that_reached_disk() {
    let workdir = tempfile::tempdir().unwrap();
    std::fs::write(workdir.path().join("blocker"), "not a directory").unwrap();
    let patch = "\
*** Begin Patch
*** Add File: a.txt
+first
*** Add File: b.txt
+second
*** Add File: blocker/never.txt
+third
*** End Patch
";
    let mut harness = HarnessBuilder::new()
        .calls(vec![
            chunks::tool_turn("patch-1", "ApplyPatch", serde_json::json!({"patch": patch})),
            chunks::text_turn("handled partial failure"),
        ])
        .messages(vec![Message::user("apply the patch")])
        .cwd(workdir.path())
        .build()
        .await;
    let recorded_turns = harness.recorded_messages.clone();
    let mut runner = harness.runner;

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "handled partial failure");

    let canonical_root = std::fs::canonicalize(workdir.path()).unwrap();
    let expected = vec![
        canonical_root.join("a.txt").display().to_string(),
        canonical_root.join("b.txt").display().to_string(),
    ];
    assert_eq!(
        std::fs::read_to_string(workdir.path().join("a.txt")).unwrap(),
        "first\n"
    );
    assert_eq!(
        std::fs::read_to_string(workdir.path().join("b.txt")).unwrap(),
        "second\n"
    );
    assert!(!workdir.path().join("blocker/never.txt").exists());

    let mut result_metadata = None;
    let mut diff_summary = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline
        && (result_metadata.is_none() || diff_summary.is_none())
    {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let Ok(Some(event)) = tokio::time::timeout(remaining, harness.event_rx.recv()).await else {
            break;
        };
        match event.payload {
            AgentEventPayload::ToolResult {
                id,
                is_error: true,
                metadata,
                ..
            } if id == "patch-1" => result_metadata = metadata,
            AgentEventPayload::TurnDiffSummary { modified_files } => {
                diff_summary = Some(modified_files)
            }
            _ => {}
        }
    }

    assert_eq!(
        result_metadata,
        Some(ToolResultMetadata::modified_files(expected.clone()))
    );
    assert_eq!(diff_summary, Some(expected.clone()));

    // The side-effect metadata must survive the authoritative turn store, not
    // only the transient ToolResult event seen by DiffTracker.
    let calls = recorded_turns.lock().unwrap();
    let second_call = calls
        .get(1)
        .expect("tool result must trigger a follow-up LLM call");
    let stored_metadata = second_call
        .iter()
        .flat_map(|turn| &turn.body.steps)
        .find_map(|step| match step {
            TurnStep::ToolBatch(batch) => batch.items.iter().find_map(|item| match &item.state {
                ToolExecState::Done(result) => result.metadata.as_ref(),
                _ => None,
            }),
            _ => None,
        });
    assert_eq!(
        stored_metadata,
        Some(&ToolResultMetadata::modified_files(expected))
    );
}
