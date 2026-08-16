#[tokio::test]
async fn exact_workflow_permission_prompts_despite_direct_grant() {
    let (events, event_rx) = mpsc::channel(32);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    hub.lock()
        .await
        .set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    let _event_loop = start_event_loop(hub.clone(), event_rx);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");

    let direct = {
        let agent = agent.clone();
        tokio::spawn(async move {
            agent
                .send_request(methods::AGENT_PERMISSION.name, request("direct", None))
                .await
        })
    };
    let (direct_token, direct_digest, _, _) = pending(&hub, "direct").await;
    ui.client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "main",
                "tool_call_id": direct_token,
                "permission_intent_digest": direct_digest,
                "allow": true,
                "remember_session": true,
            }),
        )
        .await
        .unwrap();
    assert_eq!(direct.await.unwrap().unwrap()["allow"], true);

    let workflow_causation = workflow();
    let execution = set_workflow_authority(
        &hub,
        workflow_causation.clone(),
        PermissionMode::AskAnyWrite,
    )
    .await;
    let workflow_request = {
        let agent = agent.clone();
        let workflow = workflow_causation.clone();
        tokio::spawn(async move {
            agent
                .send_request(
                    methods::AGENT_PERMISSION.name,
                    request("workflow", Some(workflow)),
                )
                .await
        })
    };
    let (token, digest, execution_generation, ui_generation) = pending(&hub, "workflow").await;
    assert_ne!(token, "workflow");
    assert_eq!(execution_generation, execution.connection_generation);
    assert_eq!(
        ui_generation,
        hub.lock().await.ui.capability_snapshot().generation
    );
    {
        let hub = hub.lock().await;
        let intent = &hub
            .pending_permissions
            .get(&("main".into(), "workflow".into()))
            .unwrap()
            .permission_intent;
        assert_eq!(intent.interaction_token(), token);
        assert_eq!(intent.seed().workflow(), Some(&workflow_causation));
    }

    ui.client
        .respond_permission("main", &token, Some(digest), true)
        .await;
    assert_eq!(workflow_request.await.unwrap().unwrap()["allow"], true);
}
