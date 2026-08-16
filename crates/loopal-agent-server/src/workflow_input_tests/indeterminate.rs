#[tokio::test]
async fn indeterminate_start_confirms_with_the_exact_request() {
    let env = envelope();
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let (handler, control) = handler(
        vec![Ok(plan)],
        vec![
            Err(WorkflowStartControlError::Indeterminate {
                request_id: request.request_id.clone(),
                message: "response lost".into(),
            }),
            Ok(response(&request)),
        ],
    );

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Handled
    );
    let requests = control.requests.lock().await;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
}

#[tokio::test]
async fn repeated_ambiguity_keeps_the_original_request_for_later_confirmation() {
    let env = envelope();
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let ambiguous = || WorkflowStartControlError::Indeterminate {
        request_id: request.request_id.clone(),
        message: "response lost".into(),
    };
    let (handler, control) = handler(
        vec![Ok(plan)],
        vec![Err(ambiguous()), Err(ambiguous()), Ok(response(&request))],
    );

    let error = handler.handle(&env, "first context").await.unwrap_err();
    assert!(error.contains(request.request_id.as_str()));
    assert_eq!(control.requests.lock().await.len(), 2);
    assert_eq!(
        handler.handle(&env, "different context").await.unwrap(),
        WorkflowInputDisposition::Handled
    );
    let requests = control.requests.lock().await;
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|candidate| candidate == &request));
}

#[tokio::test]
async fn rejection_during_confirmation_remains_indeterminate() {
    let env = envelope();
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let (handler, control) = handler(
        vec![Ok(plan)],
        vec![
            Err(WorkflowStartControlError::Indeterminate {
                request_id: request.request_id.clone(),
                message: "response lost".into(),
            }),
            Err(WorkflowStartControlError::Rejected("lease changed".into())),
            Err(WorkflowStartControlError::Rejected(
                "still unavailable".into(),
            )),
        ],
    );

    let first_error = handler.handle(&env, "").await.unwrap_err();
    let second_error = handler.handle(&env, "changed context").await.unwrap_err();

    assert!(first_error.contains("indeterminate"));
    assert!(first_error.contains(request.request_id.as_str()));
    assert!(second_error.contains("indeterminate"));
    assert!(second_error.contains(request.request_id.as_str()));
    let requests = control.requests.lock().await;
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|candidate| candidate == &request));
}

#[tokio::test]
async fn pending_start_capacity_fails_closed_without_dispatching() {
    let env = envelope();
    let plan = workflow_plan();
    let template = request_for(&env, &plan);
    let (handler, control) = handler(vec![Ok(plan)], Vec::new());
    {
        let mut pending = handler.pending_starts.lock().await;
        for index in 0..super::MAX_CACHED_DECISIONS {
            let envelope_id = uuid::Uuid::from_u128(index as u128 + 1);
            let mut request = template.clone();
            request.request_id =
                loopal_protocol::WorkflowRequestId::new(format!("human_capacity_{index}"));
            pending.insert(envelope_id, request);
        }
    }

    let error = handler.handle(&env, "").await.unwrap_err();

    assert_eq!(error, "too many indeterminate workflow starts");
    assert!(control.requests.lock().await.is_empty());
}
