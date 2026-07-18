use serde_json::json;

use crate::support::CliHarness;

/// The vault safety contract through a full turn: the model emits a Bash call
/// whose `command` holds a `<secret_ref:NAME>` placeholder; the tool pipeline
/// resolves plaintext via `hub/secret/get` IPC (harness vault), the shell runs
/// with the real value, and the result is redacted back to the wire form
/// before it reaches events, persistence, or the next LLM request.
///
/// The command proves plaintext injection by length (12) instead of embedding
/// the plaintext itself — otherwise the test's own conversation history would
/// contain the secret and void the no-plaintext assertions.
#[tokio::test]
async fn secret_resolves_in_tool_and_redacts_before_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "secret_tool",
        "calls": [
            {"expect": {"userContains": "use the secret"},
             "chunks": [
                {"type": "tool_use", "id": "s1", "name": "Bash",
                 "input": {"command":
                    "echo 'tag-<secret_ref:e2e_token>-tag'; \
                     test \"$(printf %s '<secret_ref:e2e_token>' | wc -c)\" -eq 12 \
                     && echo len-ok || echo len-bad"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "s1"},
             "chunks": [{"type": "text", "text": "secret handled"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.vault().insert("e2e_token", "s3cr3t-plain");

    let out = h.run_turn("please use the secret").await;
    assert!(
        out.error.is_none() && out.finished,
        "turn failed: {:?}\nevents: {:?}",
        out.error,
        out.events
    );
    assert!(
        out.text.contains("secret handled"),
        "follow-up text missing; text: {:?}\nevents: {:?}",
        out.text,
        out.events
    );

    let tool_result = out
        .events
        .iter()
        .find(|e| e.starts_with("ToolResult"))
        .expect("a ToolResult event");
    assert!(
        tool_result.contains("len-ok"),
        "len-ok proves the shell saw the 12-char plaintext, not the literal \
         placeholder; result: {tool_result}"
    );
    assert!(
        tool_result.contains("tag-<secret_ref:e2e_token>-tag"),
        "the echoed plaintext must be redacted back to the wire form; \
         result: {tool_result}"
    );
    assert!(
        !out.events.iter().any(|e| e.contains("s3cr3t-plain")),
        "plaintext must never appear in any agent event; events: {:?}",
        out.events
    );

    let gets = h.vault().gets();
    assert!(
        !gets.is_empty() && gets.iter().all(|g| g["name"] == "e2e_token"),
        "resolution must fetch exactly this secret from the Hub vault; \
         gets: {gets:?}"
    );

    let journal = h.journal().await.to_string();
    assert!(
        !journal.contains("s3cr3t-plain"),
        "the LLM wire must only ever see placeholders; journal: {journal}"
    );
}
