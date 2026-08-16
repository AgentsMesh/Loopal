use super::*;

#[tokio::test]
async fn run_deadline_contains_late_preparation_without_binding_it() {
    let run_id = WorkflowRunId::new("wrun_deadline_pending");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_deadline_pending")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), single_node_request("wreq_deadline_pending"))
        .await
        .unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare { response, .. } = control.next().await else {
        panic!("expected prepare effect")
    };

    handle.tick(60_100).await.unwrap();
    assert!(matches!(
        last_payload(&journal),
        WorkflowEventPayload::AttemptStopRequested { .. }
    ));
    let SpawnerEffect::AbortPrepare {
        response: abort, ..
    } = control.next().await
    else {
        panic!("expected preparation abort")
    };
    let (worker, outcome) = prepared_worker("late-worker", 11);
    assert!(response.send(Ok(worker)).is_ok());
    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected late-worker containment")
    };
    assert_eq!(execution.connection_generation, 11);
    assert!(journal.events().into_iter().all(|(_, _, events)| {
        !events
            .into_iter()
            .any(|event| matches!(event.payload, WorkflowEventPayload::AttemptBound { .. }))
    }));
    assert!(
        response
            .send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed)
            .is_ok()
    );
    assert!(outcome.is_closed());
    assert!(
        abort
            .send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed)
            .is_ok()
    );
    journal.wait_for_event_batches(4).await;

    let run = get_run(&handle, owner, run_id, "wreq_deadline_pending_get").await;
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.attempts.len(), 1);
    assert_eq!(run.failure.unwrap().class, WorkflowFailureClass::Permanent);
    drop(handle);
    task.await.unwrap();
}
