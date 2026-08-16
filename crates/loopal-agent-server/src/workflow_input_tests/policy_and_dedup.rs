#[tokio::test]
async fn explicit_policy_never_invokes_the_proactive_planner() {
    let env = envelope();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let settings = WorkflowSettings {
        execution_enabled: true,
        policy: OrchestrationPolicy::Explicit,
        ..Default::default()
    };
    let (handler, control) = planner_handler(settings, calls.clone());

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Direct
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(control.requests.lock().await.is_empty());
}

#[tokio::test]
async fn execution_disabled_never_invokes_the_proactive_planner() {
    let env = envelope();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let settings = WorkflowSettings {
        policy: OrchestrationPolicy::Proactive,
        ..Default::default()
    };
    let (handler, control) = planner_handler(settings, calls.clone());

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Direct
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(control.requests.lock().await.is_empty());
}

#[tokio::test]
async fn concurrent_duplicate_delivery_is_single_flight() {
    let env = envelope();
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let (handler, control) = handler_with_delay(
        vec![Ok(plan)],
        vec![Ok(response(&request))],
        Some(Duration::from_millis(10)),
    );

    let first_env = env.clone();
    let first_handler = handler.clone();
    let first = tokio::spawn(async move { first_handler.handle(&first_env, "").await });
    let second = handler.handle(&env, "");
    let (first, second) = tokio::join!(first, second);

    assert_eq!(first.unwrap().unwrap(), WorkflowInputDisposition::Handled);
    assert_eq!(second.unwrap(), WorkflowInputDisposition::Handled);
    assert_eq!(control.requests.lock().await.len(), 1);
}

#[tokio::test]
async fn duplicate_delivery_starts_one_workflow_with_stable_request_id() {
    let env = envelope();
    let expected_id = format!("human_{}", env.id.simple());
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let (handler, control) = handler(vec![Ok(plan)], vec![Ok(response(&request))]);

    let first = handler.handle(&env, "").await.unwrap();
    let second = handler
        .handle(&env, "ignored duplicate context")
        .await
        .unwrap();

    assert_eq!(first, WorkflowInputDisposition::Handled);
    assert_eq!(second, WorkflowInputDisposition::Handled);
    let requests = control.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request_id.as_str(), expected_id);
    assert!(requests[0].request_id.is_valid());
}

#[tokio::test]
async fn rejected_start_falls_back_direct_and_is_cached() {
    let env = envelope();
    let plan = workflow_plan();
    let (handler, control) = handler(
        vec![Ok(plan)],
        vec![Err(WorkflowStartControlError::Rejected(
            "invalid graph".into(),
        ))],
    );

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Direct
    );
    assert_eq!(
        handler.handle(&env, "duplicate").await.unwrap(),
        WorkflowInputDisposition::Direct
    );
    assert_eq!(control.requests.lock().await.len(), 1);
}
