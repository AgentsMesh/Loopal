use serde_json::json;

use crate::support::CliHarness;

/// The AskUser question loop over the wire: the model poses an options
/// question, the user seat answers via `agent/question`, and the chosen label
/// flows back into the tool result the model continues from.
#[tokio::test]
async fn ask_user_question_round_trips_the_users_answer() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "ask_user",
        "calls": [
            {"expect": {"userContains": "pick a color"},
             "chunks": [
                {"type": "tool_use", "id": "q1", "name": "AskUser",
                 "input": {"questions": [{
                    "question": "Which accent color should the theme use?",
                    "options": [
                        {"label": "cerulean", "description": "the sky one"},
                        {"label": "crimson", "description": "the warm one"}
                    ]
                 }]}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "q1"},
             "chunks": [{"type": "text", "text": "color chosen"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.permissions().set_question_answers(&["cerulean"]);

    let out = h.run_turn("please pick a color").await;
    assert!(
        out.error.is_none() && out.finished && out.text.contains("color chosen"),
        "turn failed: {out:?}"
    );
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("cerulean")),
        "the chosen answer must reach the model via the tool result; \
         events: {:?}",
        out.events
    );

    let asks = h.permissions().question_asks();
    assert_eq!(asks.len(), 1, "one question ask expected; got {asks:?}");
    assert!(
        asks[0].to_string().contains("accent color"),
        "the ask must carry the question text; ask: {}",
        asks[0]
    );
}
