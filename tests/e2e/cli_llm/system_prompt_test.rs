use serde_json::json;

use crate::support::CliHarness;

/// Project instructions actually reach the model: `.loopal/LOOPAL.md` content
/// must appear in the system prompt of the outgoing LLM request (journal
/// `systemText`), alongside the agent's cwd.
#[tokio::test]
async fn project_instructions_reach_the_system_prompt_on_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "system_prompt",
        "calls": [
            {"expect": {"userContains": "hello there"},
             "chunks": [{"type": "text", "text": "hi"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    std::fs::write(
        h.cwd().join("LOOPAL.md"),
        "Always answer like a lighthouse keeper.\nLOOPAL-E2E-INSTRUCTION-MARKER\n",
    )
    .unwrap();

    let out = h.run_turn("hello there").await;
    assert!(out.finished && out.text.contains("hi"), "turn: {out:?}");

    let journal = h.journal().await;
    let system = journal[0]["systemText"].as_str().unwrap_or_default();
    assert!(
        journal[0]["hasSystem"].as_bool().unwrap_or(false),
        "the request must carry a system prompt; journal: {journal}"
    );
    assert!(
        system.contains("LOOPAL-E2E-INSTRUCTION-MARKER"),
        "project LOOPAL.md instructions must be injected into the system \
         prompt over the wire; systemText: {system}"
    );
    assert!(
        system.contains(&h.cwd().to_string_lossy().to_string()),
        "the system prompt must state the agent's cwd; systemText: {system}"
    );
}
