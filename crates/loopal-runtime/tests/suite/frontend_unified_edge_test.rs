use std::sync::Arc;

use loopal_protocol::{AgentEventPayload, UserQuestionResponse};
use loopal_runtime::frontend::{
    PermissionHandler, QuestionHandler, RelayPermissionHandler, RelayQuestionHandler,
};
use loopal_tool_api::PermissionDecision;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_relay_permission_handler_approved() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (perm_tx, perm_rx) = mpsc::channel(16);

    let handler = Arc::new(RelayPermissionHandler::new(event_tx, perm_rx));
    let handler_clone = Arc::clone(&handler);

    tokio::spawn(async move {
        let ev = event_rx.recv().await.unwrap();
        assert!(matches!(
            ev.payload,
            AgentEventPayload::ToolPermissionRequest { .. }
        ));
        perm_tx.send(true).await.unwrap();
    });

    let d = handler_clone
        .decide("id1", "Write", &serde_json::json!({}))
        .await;
    assert_eq!(d, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_relay_permission_handler_denied() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (perm_tx, perm_rx) = mpsc::channel(16);

    let handler = RelayPermissionHandler::new(event_tx, perm_rx);

    tokio::spawn(async move {
        let _ = event_rx.recv().await;
        perm_tx.send(false).await.unwrap();
    });

    let d = handler.decide("id1", "Write", &serde_json::json!({})).await;
    assert_eq!(d, PermissionDecision::Deny);
}

#[tokio::test]
async fn test_relay_permission_handler_closed_channel_denies() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_perm_tx, perm_rx) = mpsc::channel(16);
    drop(event_rx); // close the event receiver

    let handler = RelayPermissionHandler::new(event_tx, perm_rx);
    let d = handler.decide("id1", "Write", &serde_json::json!({})).await;
    assert_eq!(d, PermissionDecision::Deny);
}

// ── RelayQuestionHandler tests ──────────────────────────────────

#[tokio::test]
async fn test_relay_question_handler_returns_answers() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (resp_tx, resp_rx) = mpsc::channel(16);

    let handler = RelayQuestionHandler::new(event_tx, resp_rx);

    tokio::spawn(async move {
        let ev = event_rx.recv().await.unwrap();
        assert!(matches!(
            ev.payload,
            AgentEventPayload::UserQuestionRequest { .. }
        ));
        resp_tx
            .send(UserQuestionResponse {
                answers: vec!["yes".into(), "42".into()],
            })
            .await
            .unwrap();
    });

    let questions = vec![loopal_protocol::Question {
        question: "Continue?".into(),
        options: vec![],
        allow_multiple: false,
    }];
    let answers = handler.ask(questions).await;
    assert_eq!(answers, vec!["yes", "42"]);
}

#[tokio::test]
async fn test_relay_question_handler_closed_channel() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_resp_tx, resp_rx) = mpsc::channel::<UserQuestionResponse>(16);
    drop(event_rx);

    let handler = RelayQuestionHandler::new(event_tx, resp_rx);
    let answers = handler.ask(vec![]).await;
    assert_eq!(answers, vec!["(channel closed)"]);
}
