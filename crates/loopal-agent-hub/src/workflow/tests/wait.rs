use std::collections::HashMap;

use loopal_protocol::{
    MAX_WORKFLOW_WAIT_MS, QualifiedAddress, WorkflowRequestError, WorkflowRequestId, WorkflowRunId,
    WorkflowRunSnapshot, WorkflowWaitRequest,
};
use tokio::sync::{mpsc, watch};

use super::super::command::WorkflowCommand;
use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorHandle};
use super::support::{owner, spec};

fn request(request_id: &str, timeout_ms: u64) -> WorkflowWaitRequest {
    WorkflowWaitRequest {
        request_id: WorkflowRequestId::new(request_id),
        run_id: WorkflowRunId::new("wrun_wait"),
        after_revision: 1,
        timeout_ms,
    }
}

#[test]
fn wait_request_accepts_the_protocol_timeout_boundary() {
    let boundary = request("wreq_wait_boundary", MAX_WORKFLOW_WAIT_MS);
    assert_eq!(super::super::wait::validate_request(&boundary), Ok(()));
    assert_eq!(
        super::super::wait::validate_request(&request("", 0)),
        Err(WorkflowCoordinatorError::Request(
            WorkflowRequestError::InvalidRequestId,
        ))
    );
}

#[tokio::test]
async fn oversized_wait_is_rejected_before_subscribing() {
    let (commands, mut receiver) = mpsc::channel(1);
    let handle = WorkflowCoordinatorHandle { commands };

    assert_eq!(
        handle
            .wait(
                owner("session", "root"),
                request("wreq_wait_too_long", MAX_WORKFLOW_WAIT_MS + 1),
            )
            .await,
        Err(WorkflowCoordinatorError::WaitTimeoutExceeded)
    );
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn valid_wait_is_connected_to_the_coordinator_subscription() {
    let (commands, mut receiver) = mpsc::channel(1);
    let handle = WorkflowCoordinatorHandle { commands };
    let waiter = tokio::spawn(async move {
        handle
            .wait(owner("session", "root"), request("wreq_wait_connected", 0))
            .await
    });

    let WorkflowCommand::Subscribe {
        owner: subscribed_owner,
        run_id,
        response,
    } = receiver.recv().await.expect("wait must subscribe")
    else {
        panic!("wait sent an unexpected coordinator command");
    };
    assert_eq!(subscribed_owner, owner("session", "root"));
    assert_eq!(run_id, WorkflowRunId::new("wrun_wait"));
    response.send(Ok(None)).unwrap();

    let response = waiter.await.unwrap().unwrap();
    assert_eq!(
        response.status,
        loopal_protocol::WorkflowWaitStatus::NotFound
    );
    assert!(response.run.is_none());
}

#[test]
fn revision_publication_is_scoped_to_the_exact_owner() {
    let run_id = WorkflowRunId::new("wrun_shared");
    let owner_a = owner("session-a", "root");
    let owner_b = owner("session-b", "root");
    let initial = snapshot(run_id.clone(), 1);
    let published = snapshot(run_id.clone(), 2);
    let (sender_a, receiver_a) = watch::channel(initial.clone());
    let (sender_b, receiver_b) = watch::channel(initial);
    let mut senders = HashMap::from([
        ((owner_a.clone(), run_id.clone()), sender_a),
        ((owner_b, run_id), sender_b),
    ]);

    super::super::wait::publish(&mut senders, &owner_a, &published);

    assert_eq!(receiver_a.borrow().revision, 2);
    assert_eq!(receiver_b.borrow().revision, 1);
}

fn snapshot(run_id: WorkflowRunId, revision: u64) -> WorkflowRunSnapshot {
    let mut snapshot =
        WorkflowRunSnapshot::planned(run_id, QualifiedAddress::local("root"), spec(), 1);
    snapshot.revision = revision;
    snapshot
}
