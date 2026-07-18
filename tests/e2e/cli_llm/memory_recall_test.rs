use serde_json::json;

use crate::support::CliHarness;

/// Project memory through a real turn: a source-of-truth note under
/// `.loopal/memory/` is indexed at agent start, and a `memory_recall` call
/// matching only the note's BODY text (full-body FTS) returns its content to
/// the model.
#[tokio::test]
async fn memory_recall_finds_note_body_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "memory_recall",
        "calls": [
            {"expect": {"userContains": "recall the deploy"},
             "chunks": [
                {"type": "tool_use", "id": "mr1", "name": "memory_recall",
                 "input": {"query": "tide-gate"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "mr1"},
             "chunks": [{"type": "text", "text": "memory recalled"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let memory_dir = h.cwd().join(".loopal/memory");
    std::fs::create_dir_all(&memory_dir).unwrap();
    std::fs::write(
        memory_dir.join("deploy-ritual.md"),
        "---\nname: deploy-ritual\ndescription: How this project deploys\n---\n\n\
         Deploys run through the tide-gate script every Thursday.\n",
    )
    .unwrap();

    let out = h.run_turn("please recall the deploy").await;
    assert!(
        out.finished && out.text.contains("memory recalled"),
        "turn failed: {out:?}"
    );
    let result = out
        .events
        .iter()
        .find(|e| e.starts_with("ToolResult"))
        .expect("a memory_recall ToolResult");
    assert!(
        result.contains("deploy-ritual") && result.contains("tide-gate"),
        "a body-only keyword must find the note and return its content; \
         result: {result}"
    );
}
