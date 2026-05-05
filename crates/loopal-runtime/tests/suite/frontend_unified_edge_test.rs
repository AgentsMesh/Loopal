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
        let id = match ev.payload {
            AgentEventPayload::UserQuestionRequest { id, .. } => id,
            _ => panic!("expected UserQuestionRequest"),
        };
        resp_tx
            .send(UserQuestionResponse::answered(
                &id,
                vec!["yes".to_string(), "42".to_string()],
            ))
            .await
            .unwrap();
    });

    let questions = vec![loopal_protocol::Question {
        question: "Continue?".into(),
        options: vec![],
        allow_multiple: false,
    }];
    let response = handler.ask(questions).await;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers.len(), 2);
            assert_eq!(answers[0], "yes");
            assert_eq!(answers[1], "42");
        }
        other => panic!("expected Answered, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_relay_question_handler_closed_channel() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_resp_tx, resp_rx) = mpsc::channel::<UserQuestionResponse>(16);
    drop(event_rx);

    let handler = RelayQuestionHandler::new(event_tx, resp_rx);
    let response = handler.ask(vec![]).await;
    assert!(matches!(response, UserQuestionResponse::Cancelled { .. }));
}

#[tokio::test]
async fn test_relay_question_handler_discards_stale_id() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (resp_tx, resp_rx) = mpsc::channel(16);

    let handler = RelayQuestionHandler::new(event_tx, resp_rx);

    tokio::spawn(async move {
        let ev = event_rx.recv().await.unwrap();
        let id = match ev.payload {
            AgentEventPayload::UserQuestionRequest { id, .. } => id,
            _ => panic!("expected UserQuestionRequest"),
        };
        // first send a stale (different) id — should be discarded
        resp_tx
            .send(UserQuestionResponse::answered(
                "OTHER-ID",
                vec!["stale".to_string()],
            ))
            .await
            .unwrap();
        // then send the real one
        resp_tx
            .send(UserQuestionResponse::answered(
                &id,
                vec!["fresh".to_string()],
            ))
            .await
            .unwrap();
    });

    let questions = vec![loopal_protocol::Question {
        question: "Q?".into(),
        options: vec![],
        allow_multiple: false,
    }];
    let response = handler.ask(questions).await;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers, vec!["fresh".to_string()]);
        }
        other => panic!("expected fresh Answered, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_relay_question_handler_drains_after_pre_existing_stale() {
    // 模拟"上一个 ask 的迟到响应已经堆在 channel 里"场景：
    // 新 ask 应该 discard stale + 接受匹配 id 的响应。
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (resp_tx, resp_rx) = mpsc::channel(16);

    // 预先塞两条陈旧响应（不同 id）
    resp_tx
        .send(UserQuestionResponse::answered(
            "OLD-ID-1",
            vec!["stale1".to_string()],
        ))
        .await
        .unwrap();
    resp_tx
        .send(UserQuestionResponse::answered(
            "OLD-ID-2",
            vec!["stale2".to_string()],
        ))
        .await
        .unwrap();

    let handler = RelayQuestionHandler::new(event_tx, resp_rx);

    tokio::spawn(async move {
        let ev = event_rx.recv().await.unwrap();
        let id = match ev.payload {
            AgentEventPayload::UserQuestionRequest { id, .. } => id,
            _ => panic!("expected UserQuestionRequest"),
        };
        resp_tx
            .send(UserQuestionResponse::answered(
                &id,
                vec!["fresh".to_string()],
            ))
            .await
            .unwrap();
    });

    let response = handler.ask(vec![]).await;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers, vec!["fresh".to_string()]);
        }
        other => panic!("expected fresh Answered, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_relay_question_handler_accepts_empty_id_as_self_sentinel() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (resp_tx, resp_rx) = mpsc::channel(16);

    let handler = RelayQuestionHandler::new(event_tx, resp_rx);

    tokio::spawn(async move {
        let _ev = event_rx.recv().await.unwrap();
        // Frontend IPC fallback constructs cancelled with empty id —
        // RelayQuestionHandler must treat this as self-sentinel, not discard.
        resp_tx
            .send(UserQuestionResponse::cancelled(""))
            .await
            .unwrap();
    });

    let response = handler.ask(vec![]).await;
    match response {
        UserQuestionResponse::Cancelled { question_id } => {
            assert!(
                !question_id.is_empty(),
                "self-sentinel should be rewritten with local id"
            );
        }
        other => panic!("expected Cancelled, got: {other:?}"),
    }
}
