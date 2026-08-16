use agent_client_protocol_schema::StopReason;
use loopal_ipc::connection::Incoming;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, Question};
use serde_json::json;

use super::{harness, read_request};

#[tokio::test]
async fn question_event_routes_answer_and_loop_continues() {
    let mut harness = harness();
    let loop_task = tokio::spawn({
        let adapter = harness.adapter.clone();
        async move { adapter.run_event_loop("session-1").await }
    });
    harness
        .events
        .send(AgentEvent::named(
            QualifiedAddress::local("worker"),
            AgentEventPayload::UserQuestionRequest {
                id: "question-1".into(),
                logical_id: "logical-1".into(),
                questions: vec![Question {
                    question: "Choose".into(),
                    options: vec![],
                    allow_multiple: false,
                    header: None,
                }],
                classifier_running: false,
            },
        ))
        .unwrap();
    let request = read_request(&mut harness.acp_reader).await;
    assert_eq!(request["method"], "_loopal/question");
    harness
        .adapter
        .acp_out
        .route_response(request["id"].as_i64().unwrap(), json!({"answers":["yes"]}))
        .await;
    let Incoming::Request { id, method, params } = harness.hub_rx.recv().await.unwrap() else {
        panic!("expected question response");
    };
    assert_eq!(method, "hub/question_response");
    assert_eq!(params["agent_name"], "worker");
    assert_eq!(params["response"]["answers"], json!(["yes"]));
    harness.hub.respond(id, json!({})).await.unwrap();
    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();
    assert_eq!(loop_task.await.unwrap(), StopReason::EndTurn);
}

#[tokio::test]
async fn question_event_defaults_agent_and_cancels_empty_answer() {
    let mut harness = harness();
    let loop_task = tokio::spawn({
        let adapter = harness.adapter.clone();
        async move { adapter.run_event_loop("session-1").await }
    });
    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::UserQuestionRequest {
            id: "question-2".into(),
            logical_id: "logical-2".into(),
            questions: vec![],
            classifier_running: false,
        }))
        .unwrap();
    let request = read_request(&mut harness.acp_reader).await;
    harness
        .adapter
        .acp_out
        .route_response(request["id"].as_i64().unwrap(), json!({"answers":[]}))
        .await;
    let Incoming::Request { id, params, .. } = harness.hub_rx.recv().await.unwrap() else {
        panic!("expected question cancellation");
    };
    assert_eq!(params["agent_name"], "main");
    assert_eq!(params["response"]["kind"], "cancelled");
    harness.hub.respond(id, json!({})).await.unwrap();
    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::AwaitingInput))
        .unwrap();
    assert_eq!(loop_task.await.unwrap(), StopReason::EndTurn);
}

#[tokio::test]
async fn mode_and_resolved_events_emit_notifications() {
    let mut harness = harness();
    let mode = AgentEvent::root(AgentEventPayload::ModeChanged {
        mode: "plan".into(),
    });
    assert_eq!(harness.adapter.handle_event(&mode, "session-1").await, None);
    assert_eq!(
        read_request(&mut harness.acp_reader).await["method"],
        "session/update"
    );
    let extension = read_request(&mut harness.acp_reader).await;
    assert_eq!(extension["method"], "_loopal/mode");
    assert_eq!(extension["params"]["data"]["mode"], "plan");

    let resolved = AgentEvent::root(AgentEventPayload::ToolPermissionResolved {
        id: "permission-1".into(),
    });
    assert_eq!(
        harness.adapter.handle_event(&resolved, "session-1").await,
        None
    );
    let extension = read_request(&mut harness.acp_reader).await;
    assert_eq!(extension["method"], "_loopal/permission_resolved");
    assert_eq!(extension["params"]["toolCallId"], "permission-1");
}

#[tokio::test]
async fn child_finished_and_stream_do_not_end_or_pollute_root_prompt() {
    let mut harness = harness();
    let loop_task = tokio::spawn({
        let adapter = harness.adapter.clone();
        async move { adapter.run_event_loop("session-1").await }
    });
    harness
        .events
        .send(AgentEvent::named(
            QualifiedAddress::local("worker"),
            AgentEventPayload::Stream {
                text: "child output".into(),
            },
        ))
        .unwrap();
    harness
        .events
        .send(AgentEvent::named(
            QualifiedAddress::local("worker"),
            AgentEventPayload::Finished,
        ))
        .unwrap();
    tokio::task::yield_now().await;
    assert!(!loop_task.is_finished());
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(20),
            read_request(&mut harness.acp_reader),
        )
        .await
        .is_err(),
        "child stream must not be projected into the root ACP session",
    );

    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();
    assert_eq!(loop_task.await.unwrap(), StopReason::EndTurn);
}
