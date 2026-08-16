use loopal_agent::workflow_control::WorkflowStartControlError;
use loopal_protocol::{Envelope, MessageSource, WorkflowStartLookupResponse};
use loopal_runtime::workflow_input::{WorkflowInputDisposition, WorkflowInputHandler};

use super::MAX_CACHED_DECISIONS;
use super::test_support::*;

#[tokio::test]
async fn evicted_handled_envelope_uses_durable_lookup_without_replanning() {
    let env = envelope();
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let existing = response(&request);
    let (handler, control) = handler(vec![Ok(plan)], vec![Ok(existing.clone())]);

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Handled
    );
    for index in 0..MAX_CACHED_DECISIONS {
        let filler = Envelope::new(MessageSource::Human, "main", format!("small task {index}"));
        assert_eq!(
            handler.handle(&filler, "").await.unwrap(),
            WorkflowInputDisposition::Direct
        );
    }
    control
        .lookups
        .lock()
        .await
        .push_back(Ok(WorkflowStartLookupResponse::Found {
            response: existing,
        }));

    assert_eq!(
        handler.handle(&env, "changed context").await.unwrap(),
        WorkflowInputDisposition::Handled
    );
    assert_eq!(control.requests.lock().await.len(), 1);
    assert_eq!(
        control.lookup_requests.lock().await.len(),
        MAX_CACHED_DECISIONS + 2
    );
}

#[tokio::test]
async fn fresh_handler_replays_existing_start_without_invoking_planner() {
    let env = envelope();
    let plan = workflow_plan();
    let request = request_for(&env, &plan);
    let (handler, control) = handler(Vec::new(), Vec::new());
    control
        .lookups
        .lock()
        .await
        .push_back(Ok(WorkflowStartLookupResponse::Found {
            response: response(&request),
        }));

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Handled
    );
    assert!(control.requests.lock().await.is_empty());
    assert_eq!(
        control.lookup_requests.lock().await[0].request_id,
        request.request_id
    );
}

#[tokio::test]
async fn payload_mismatch_cannot_downgrade_an_existing_start_to_direct() {
    let env = envelope();
    let original_plan = workflow_plan();
    let original_request = request_for(&env, &original_plan);
    let existing = response(&original_request);
    let mut changed: loopal_protocol::WorkflowPlanDecision =
        serde_json::from_str(&original_plan).unwrap();
    let loopal_protocol::WorkflowExecution::Workflow { spec } = &mut changed.execution else {
        unreachable!()
    };
    spec.run_goal = "a different replanned workflow".into();
    let (handler, control) = handler(
        vec![Ok(serde_json::to_string(&changed).unwrap())],
        vec![Err(WorkflowStartControlError::Rejected(
            "workflow request payload mismatch".into(),
        ))],
    );
    control.lookups.lock().await.extend([
        Ok(WorkflowStartLookupResponse::NotFound),
        Ok(WorkflowStartLookupResponse::Found { response: existing }),
    ]);

    assert_eq!(
        handler.handle(&env, "").await.unwrap(),
        WorkflowInputDisposition::Handled
    );
    let requests = control.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_ne!(requests[0].spec, original_request.spec);
}

#[tokio::test]
async fn lookup_operation_conflict_fails_closed_before_planning() {
    let env = envelope();
    let (handler, control) = handler(Vec::new(), Vec::new());
    control
        .lookups
        .lock()
        .await
        .push_back(Ok(WorkflowStartLookupResponse::Conflict));

    let error = handler.handle(&env, "").await.unwrap_err();

    assert!(error.contains("different operation"), "{error}");
    assert!(control.requests.lock().await.is_empty());
}
