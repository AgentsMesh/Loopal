#[tokio::test]
async fn stale_permission_execution_is_denied_before_any_ui_or_grant_effect() {
    let (hub, _peer, connection, execution) =
        exact_connection_fixture(Arc::new(loopal_vault_api::NoopAuditSink)).await;
    let stale = AgentExecutionRef::local(
        execution.address.agent.clone(),
        execution.connection_generation + 1,
    );

    super::handle_agent_permission(&hub, connection, 41, request("stale", None), "main", stale)
        .await;

    assert!(hub.lock().await.pending_permissions.is_empty());
    assert_eq!(hub.lock().await.permission_receipts.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remembered_grant_revoked_during_audit_fails_closed_before_receipt() {
    let (sink, gate) = Sink::gated();
    let (hub, _peer, connection, execution) = exact_connection_fixture(Arc::new(sink)).await;
    let permission = PermissionIntentRequest::create(
        "revoked-during-audit",
        "Write",
        json!({"file_path": "one"}),
        json!({"file_path": "one"}),
        json!({"type": "object", "required": ["file_path"]}),
        None,
    )
    .unwrap();
    hub.lock()
        .await
        .grant_permission(execution.clone(), &permission.intent_seed);
    let handling = tokio::spawn({
        let hub = hub.clone();
        let connection = connection.clone();
        let execution = execution.clone();
        async move {
            super::handle_agent_permission(
                &hub,
                connection,
                42,
                serde_json::to_value(permission).unwrap(),
                "main",
                execution,
            )
            .await;
        }
    });
    gate.wait_started().await;
    hub.lock().await.clear_permission_grants(&execution);
    gate.release();
    handling.await.unwrap();

    assert_eq!(hub.lock().await.permission_receipts.len(), 0);
    assert!(hub.lock().await.pending_permissions.is_empty());
}
