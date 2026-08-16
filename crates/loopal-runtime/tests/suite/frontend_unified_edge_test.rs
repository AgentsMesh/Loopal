use loopal_protocol::{AgentEventPayload, UserQuestionResponse};
use loopal_runtime::frontend::{ManualQuestionHandler, QuestionHandler};
use tokio::sync::mpsc;

#[tokio::test]
async fn question_handler_returns_answers() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (response_tx, response_rx) = mpsc::channel(16);
    let handler = ManualQuestionHandler::new(event_tx, response_rx);

    tokio::spawn(async move {
        let event = event_rx.recv().await.unwrap();
        let id = match event.payload {
            AgentEventPayload::UserQuestionRequest { id, .. } => id,
            _ => panic!("expected UserQuestionRequest"),
        };
        response_tx
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
        header: None,
    }];
    let response = handler.ask(questions).await.response;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers, vec!["yes".to_string(), "42".to_string()]);
        }
        other => panic!("expected Answered, got: {other:?}"),
    }
}

#[tokio::test]
async fn question_handler_denies_closed_event_channel() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_response_tx, response_rx) = mpsc::channel::<UserQuestionResponse>(16);
    drop(event_rx);
    let handler = ManualQuestionHandler::new(event_tx, response_rx);

    let response = handler.ask(vec![]).await.response;
    assert!(matches!(response, UserQuestionResponse::Cancelled { .. }));
}

#[tokio::test]
async fn question_handler_discards_stale_id() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (response_tx, response_rx) = mpsc::channel(16);
    let handler = ManualQuestionHandler::new(event_tx, response_rx);

    tokio::spawn(async move {
        let event = event_rx.recv().await.unwrap();
        let id = match event.payload {
            AgentEventPayload::UserQuestionRequest { id, .. } => id,
            _ => panic!("expected UserQuestionRequest"),
        };
        response_tx
            .send(UserQuestionResponse::answered(
                "OTHER-ID",
                vec!["stale".to_string()],
            ))
            .await
            .unwrap();
        response_tx
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
        header: None,
    }];
    let response = handler.ask(questions).await.response;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers, vec!["fresh".to_string()]);
        }
        other => panic!("expected fresh Answered, got: {other:?}"),
    }
}

#[tokio::test]
async fn question_handler_drains_preexisting_stale_responses() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (response_tx, response_rx) = mpsc::channel(16);
    for id in ["OLD-ID-1", "OLD-ID-2"] {
        response_tx
            .send(UserQuestionResponse::answered(
                id,
                vec!["stale".to_string()],
            ))
            .await
            .unwrap();
    }
    let handler = ManualQuestionHandler::new(event_tx, response_rx);

    tokio::spawn(async move {
        let event = event_rx.recv().await.unwrap();
        let id = match event.payload {
            AgentEventPayload::UserQuestionRequest { id, .. } => id,
            _ => panic!("expected UserQuestionRequest"),
        };
        response_tx
            .send(UserQuestionResponse::answered(
                &id,
                vec!["fresh".to_string()],
            ))
            .await
            .unwrap();
    });

    let response = handler.ask(vec![]).await.response;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers, vec!["fresh".to_string()]);
        }
        other => panic!("expected fresh Answered, got: {other:?}"),
    }
}

#[tokio::test]
async fn question_handler_accepts_empty_id_as_self_sentinel() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (response_tx, response_rx) = mpsc::channel(16);
    let handler = ManualQuestionHandler::new(event_tx, response_rx);

    tokio::spawn(async move {
        let _ = event_rx.recv().await.unwrap();
        response_tx
            .send(UserQuestionResponse::cancelled(""))
            .await
            .unwrap();
    });

    let response = handler.ask(vec![]).await.response;
    match response {
        UserQuestionResponse::Cancelled { question_id } => assert!(!question_id.is_empty()),
        other => panic!("expected Cancelled, got: {other:?}"),
    }
}
