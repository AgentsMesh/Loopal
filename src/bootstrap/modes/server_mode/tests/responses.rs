#[tokio::test]
async fn responds_to_permission_question_and_plan_with_bound_values() {
    let (client, server, mut incoming) = client_pair();
    let (event_tx, event_rx) = tokio::sync::broadcast::channel(8);
    let permission_intent = intent();
    let expected_digest = permission_intent.intent_digest().to_string();
    event_tx
        .send(AgentEvent::named(
            QualifiedAddress::local("worker"),
            AgentEventPayload::ToolPermissionRequest {
                id: "permission-1".into(),
                name: "Bash".into(),
                input: serde_json::json!({"command": "true"}),
                permission_intent: Some(Box::new(permission_intent)),
            },
        ))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::UserQuestionRequest {
            id: "question-1".into(),
            logical_id: "logical-1".into(),
            questions: vec![question("One?"), question("Two?")],
            classifier_running: false,
        }))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::PlanApprovalRequest {
            id: "plan-1".into(),
            plan_content: "# Plan".into(),
            plan_path: "/tmp/plan.md".into(),
        }))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();

    let consumer = tokio::spawn(consume_events(event_rx, client));
    let (method, params) = acknowledge(&server, &mut incoming).await;
    assert_eq!(
        method,
        loopal_ipc::protocol::methods::HUB_PERMISSION_RESPONSE.name
    );
    assert_eq!(params["agent_name"], "worker");
    assert_eq!(params["tool_call_id"], "permission-1");
    assert_eq!(params["permission_intent_digest"], expected_digest);
    assert_eq!(params["allow"], true);

    let (method, params) = acknowledge(&server, &mut incoming).await;
    assert_eq!(
        method,
        loopal_ipc::protocol::methods::HUB_QUESTION_RESPONSE.name
    );
    assert_eq!(params["agent_name"], loopal_protocol::ROOT_AGENT_NAME);
    assert_eq!(params["question_id"], "question-1");
    assert_eq!(params["response"]["answers"].as_array().unwrap().len(), 2);

    let (method, params) = acknowledge(&server, &mut incoming).await;
    assert_eq!(
        method,
        loopal_ipc::protocol::methods::HUB_PLAN_APPROVAL_RESPONSE.name
    );
    assert_eq!(params["request_id"], "plan-1");
    assert_eq!(params["decision"], "approve");
    assert!(consumer.await.unwrap().is_empty());
}

#[tokio::test]
async fn permission_without_intent_sends_no_digest() {
    let (client, server, mut incoming) = client_pair();
    let (event_tx, event_rx) = tokio::sync::broadcast::channel(4);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
            id: "legacy-permission".into(),
            name: "Bash".into(),
            input: serde_json::json!({}),
            permission_intent: None,
        }))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();

    let consumer = tokio::spawn(consume_events(event_rx, client));
    let (_, params) = acknowledge(&server, &mut incoming).await;
    assert!(params["permission_intent_digest"].is_null());
    assert!(consumer.await.unwrap().is_empty());
}
