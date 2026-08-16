use std::io::Write;
use std::sync::Arc;

use loopal_protocol::{WorkflowEvent, WorkflowEventPayload, WorkflowRunId, WorkflowRunState};
use loopal_storage::{SessionStore, WorkflowJournal};

use super::super::journal::SessionWorkflowJournals;
use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::support::{coordinator_with_storage, get_request, owner, request};

#[tokio::test]
async fn restart_restores_latest_run_and_historical_request_responses() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()));
    let owner = owner("session-real", "root");
    let run_id = WorkflowRunId::new("wrun_restart");
    let start_request = request("wreq_start");
    let historical_request = get_request("wreq_historical", run_id.clone());
    let (handle, task, _, _) = coordinator_with_storage(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [run_id.clone()],
        Arc::new(SessionWorkflowJournals::new(sessions.clone())),
    );
    let started = handle
        .start(owner.clone(), start_request.clone())
        .await
        .unwrap();
    let historical = handle
        .get(owner.clone(), historical_request.clone())
        .await
        .unwrap();
    drop(handle);
    task.await.unwrap();

    let journal =
        WorkflowJournal::from_session_store(sessions.as_ref(), &owner.session_id, run_id.clone())
            .unwrap();
    journal
        .append_commit(
            vec![WorkflowEvent {
                run_id: run_id.clone(),
                revision: 2,
                occurred_at_unix_ms: 12,
                payload: WorkflowEventPayload::RunStarted,
            }],
            None,
        )
        .unwrap();

    let (handle, task, clock, ids) = coordinator_with_storage(
        WorkflowCoordinatorMode::Preview,
        std::iter::empty::<u64>(),
        std::iter::empty::<WorkflowRunId>(),
        Arc::new(SessionWorkflowJournals::new(sessions.clone())),
    );
    assert_eq!(handle.recover(owner.clone()).await.unwrap(), 1);
    let latest = handle
        .get(owner.clone(), get_request("wreq_latest", run_id.clone()))
        .await
        .unwrap();
    assert_eq!(latest.run.as_ref().unwrap().revision, 2);
    assert_eq!(
        latest.run.as_ref().unwrap().state,
        WorkflowRunState::Running
    );
    let path = sessions
        .workflow_journal_path(&owner.session_id, run_id.as_str())
        .unwrap();
    let committed_len = std::fs::metadata(&path).unwrap().len();

    assert_eq!(
        handle.get(owner.clone(), historical_request).await.unwrap(),
        historical
    );
    assert_eq!(
        handle.start(owner.clone(), start_request).await.unwrap(),
        started
    );
    assert_eq!(std::fs::metadata(path).unwrap().len(), committed_len);
    assert_eq!(clock.calls(), 0);
    assert_eq!(ids.calls(), 0);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn restart_repairs_only_the_final_torn_tail() {
    let (temp, sessions, owner, run_id) = initialized_journal("wrun_torn").await;
    let path = sessions
        .workflow_journal_path(&owner.session_id, run_id.as_str())
        .unwrap();
    let good = std::fs::read(&path).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(br#"{"kind":"commit""#)
        .unwrap();

    let (handle, task, _, _) = recovered_coordinator(sessions.clone());
    assert_eq!(handle.recover(owner.clone()).await.unwrap(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), good);
    let journal =
        WorkflowJournal::from_session_store(sessions.as_ref(), &owner.session_id, run_id).unwrap();
    assert!(journal.replay().unwrap().torn_tail.is_none());
    drop(handle);
    task.await.unwrap();
    drop(temp);
}

#[tokio::test]
async fn restart_rejects_newline_terminated_corruption_without_mutation() {
    let (temp, sessions, owner, run_id) = initialized_journal("wrun_corrupt").await;
    let path = sessions
        .workflow_journal_path(&owner.session_id, run_id.as_str())
        .unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"not json\n")
        .unwrap();
    let corrupt = std::fs::read(&path).unwrap();

    let (handle, task, clock, ids) = recovered_coordinator(sessions);
    assert_eq!(
        handle.recover(owner.clone()).await,
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );
    assert_eq!(
        handle.start(owner, request("wreq_new")).await,
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );
    assert_eq!(std::fs::read(path).unwrap(), corrupt);
    assert_eq!(clock.calls(), 0);
    assert_eq!(ids.calls(), 0);
    drop(handle);
    task.await.unwrap();
    drop(temp);
}

async fn initialized_journal(
    run_id: &str,
) -> (
    tempfile::TempDir,
    Arc<SessionStore>,
    super::super::WorkflowOwner,
    WorkflowRunId,
) {
    let temp = tempfile::tempdir().unwrap();
    let sessions = Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()));
    let owner = owner("session-real", "root");
    let run_id = WorkflowRunId::new(run_id);
    let (handle, task, _, _) = coordinator_with_storage(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [run_id.clone()],
        Arc::new(SessionWorkflowJournals::new(sessions.clone())),
    );
    handle
        .start(owner.clone(), request("wreq_start"))
        .await
        .unwrap();
    drop(handle);
    task.await.unwrap();
    (temp, sessions, owner, run_id)
}

fn recovered_coordinator(
    sessions: Arc<SessionStore>,
) -> (
    super::super::WorkflowCoordinatorHandle,
    tokio::task::JoinHandle<()>,
    Arc<super::support::TestClock>,
    Arc<super::support::TestIds>,
) {
    coordinator_with_storage(
        WorkflowCoordinatorMode::Preview,
        std::iter::empty::<u64>(),
        std::iter::empty::<WorkflowRunId>(),
        Arc::new(SessionWorkflowJournals::new(sessions)),
    )
}
