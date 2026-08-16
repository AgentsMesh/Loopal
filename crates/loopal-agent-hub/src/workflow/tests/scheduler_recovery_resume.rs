use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowRunId};

use crate::workflow::recovery::RecoveredOwner;
use crate::workflow::tests::journal_support::TestJournal;
use crate::workflow::tests::scheduler_recovery::{recovered_quiescent_run, shutdown_with_pending};
use crate::workflow::tests::scheduler_support::{SpawnerEffect, coordinator, test_spawner};
use crate::workflow::tests::support::{owner, request};

#[tokio::test]
async fn resume_dispatches_a_recovered_validated_run() {
    let run_id = WorkflowRunId::new("wrun_resume_validated");
    let journal = Arc::new(TestJournal::new());
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![recovered_quiescent_run(run_id.clone(), false)],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..130,
        [],
        [WorkflowAttemptId::new("watt_resume_validated")],
        journal,
        spawner,
    );
    let owner = owner("session-resume-validated", "root");

    assert_eq!(handle.recover(owner.clone()).await.unwrap(), 1);
    handle.resume(owner.clone()).await.unwrap();
    let SpawnerEffect::Prepare {
        request,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected recovered validated run to be prepared")
    };
    assert_eq!(request.causation.run_id, run_id);
    assert_eq!(request.causation.node_id.as_str(), "source");

    shutdown_with_pending(handle, task, control, prepare).await;
}

#[tokio::test]
async fn resume_dispatches_a_recovered_running_ready_run() {
    let run_id = WorkflowRunId::new("wrun_resume_running");
    let journal = Arc::new(TestJournal::new());
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![recovered_quiescent_run(run_id.clone(), true)],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..130,
        [],
        [WorkflowAttemptId::new("watt_resume_running")],
        journal,
        spawner,
    );
    let owner = owner("session-resume-running", "root");

    handle.recover(owner.clone()).await.unwrap();
    handle.resume(owner).await.unwrap();
    let SpawnerEffect::Prepare {
        request,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected recovered running ready run to be prepared")
    };
    assert_eq!(request.causation.run_id, run_id);

    shutdown_with_pending(handle, task, control, prepare).await;
}

#[tokio::test]
async fn resume_is_idempotent_while_an_attempt_is_pending() {
    let run_id = WorkflowRunId::new("wrun_resume_idempotent");
    let journal = Arc::new(TestJournal::new());
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![recovered_quiescent_run(run_id, true)],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..130,
        [],
        [WorkflowAttemptId::new("watt_resume_idempotent")],
        journal,
        spawner,
    );
    let owner = owner("session-resume-idempotent", "root");

    handle.recover(owner.clone()).await.unwrap();
    handle.resume(owner.clone()).await.unwrap();
    let SpawnerEffect::Prepare {
        response: prepare, ..
    } = control.next().await
    else {
        panic!("expected pending preparation")
    };
    handle.resume(owner).await.unwrap();
    control.assert_idle().await;

    shutdown_with_pending(handle, task, control, prepare).await;
}

#[tokio::test]
async fn resumed_owner_schedules_a_new_start_follow_up() {
    let run_id = WorkflowRunId::new("wrun_resume_start");
    let journal = Arc::new(TestJournal::new());
    journal.push_recovery(Ok(RecoveredOwner {
        runs: Vec::new(),
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..130,
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_resume_start")],
        journal,
        spawner,
    );
    let owner = owner("session-resume-start", "root");
    handle.recover(owner.clone()).await.unwrap();
    handle.resume(owner.clone()).await.unwrap();

    let mut start = request("wreq_resume_start");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner, start).await.unwrap();
    let SpawnerEffect::Prepare {
        request,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected start follow-up preparation")
    };
    assert_eq!(request.causation.run_id, run_id);
    shutdown_with_pending(handle, task, control, prepare).await;
}
