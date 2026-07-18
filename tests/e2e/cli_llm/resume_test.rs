use serde_json::json;

use crate::support::CliHarness;

/// Session persistence across a hard process restart: turn one runs in the
/// first agent process, the process is killed, a fresh process resumes the
/// same session id, and the second turn's LLM request must carry the first
/// turn's history over the wire.
#[tokio::test]
async fn session_resumes_across_process_restart_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "resume",
        "calls": [
            {"expect": {"userContains": "remember alpha"},
             "chunks": [{"type": "text", "text": "noted one"}, {"type": "done"}]},
            {"expect": {"userContains": "recall please"},
             "chunks": [{"type": "text", "text": "recalled two"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let session_id = h.begin_persistent().await;
    assert!(
        !session_id.is_empty(),
        "agent_start must return a session id"
    );
    let out1 = h.turn_via_message("remember alpha").await;
    assert!(
        out1.finished && out1.text.contains("noted one"),
        "turn 1 failed: {out1:?}"
    );

    h.restart().await;
    let (resumed_id, startup) = h.resume_persistent(&session_id).await;
    assert_eq!(
        resumed_id, session_id,
        "resume must reuse the same session id"
    );
    assert!(
        startup.iter().any(|e| e.starts_with("SessionHistoryLoaded")
            && e.contains("remember alpha")
            && e.contains("noted one")),
        "resume must replay the persisted history to the client; \
         startup events: {startup:?}"
    );

    let out2 = h.turn_via_message("recall please").await;
    assert!(
        out2.finished && out2.text.contains("recalled two"),
        "post-resume turn failed: {out2:?}"
    );

    let journal = h.journal().await;
    let first = journal[0]["messageCount"].as_u64().unwrap_or(0);
    let second = journal[1]["messageCount"].as_u64().unwrap_or(0);
    assert!(
        second > first,
        "the post-resume turn must include the pre-restart history in its \
         LLM request; journal: {journal}"
    );
}
