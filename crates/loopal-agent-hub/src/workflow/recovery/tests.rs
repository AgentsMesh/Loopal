mod delivery;
mod delivery_conflicts;
mod support;
mod validation;

use loopal_protocol::{
    QualifiedAddress, WorkflowEventPayload, WorkflowGetResponse, WorkflowRunId,
    WorkflowRunSnapshot, WorkflowRunState,
};
use loopal_storage::{WorkflowJournalCommit, WorkflowJournalReplay};

use super::{WorkflowCoordinatorError, recover_owner};
use support::*;

#[test]
fn positive_get_replays_historical_snapshot_and_restores_latest_run() {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    replay.commits.push(request_commit(get_record(
        "wreq_get",
        &validated,
        Some(validated.clone()),
    )));
    let started = event(&validated, WorkflowEventPayload::RunStarted);
    replay.commits.push(event_commit(&validated.id, started));

    let recovered = recover_owner(&owner(), vec![replay]).unwrap();
    assert_eq!(recovered.runs[0].state, WorkflowRunState::Running);
    assert_eq!(recovered.runs[0].revision, 2);
    let get = recovered
        .requests
        .records()
        .iter()
        .find(|record| record.operation == "get")
        .unwrap();
    let response: WorkflowGetResponse = serde_json::from_value(get.response.clone()).unwrap();
    assert_eq!(response.run.unwrap(), validated);
}

#[test]
fn exact_duplicate_get_record_counts_once() {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    let get = get_record("wreq_get", &validated, Some(validated.clone()));
    replay.commits.push(request_commit(get.clone()));
    replay.commits.push(request_commit(get));

    let recovered = recover_owner(&owner(), vec![replay]).unwrap();
    assert_eq!(recovered.requests.records().len(), 2);
}

#[test]
fn negative_or_wrong_run_get_response_fails_recovery() {
    let cases = [
        invalid_get_case(None),
        invalid_get_case(Some(WorkflowRunSnapshot::planned(
            WorkflowRunId::new("wrun_other"),
            QualifiedAddress::local("root"),
            replay("wrun_temp", "wreq_temp").1.spec,
            1,
        ))),
    ];
    assert_invalid(cases);
}

#[test]
fn unsupported_or_mixed_request_commit_fails_recovery() {
    assert_invalid([unsupported_request_case(), mixed_commit_case()]);
}

#[test]
fn cancel_event_and_request_commit_recovers_terminal_snapshot() {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    let cancel = event(
        &validated,
        WorkflowEventPayload::CancelRequested {
            reason: Some("stop".into()),
        },
    );
    let cancelled = super::apply_event(&validated, &cancel).unwrap();
    replay.commits.push(WorkflowJournalCommit {
        run_id: validated.id.clone(),
        events: vec![cancel],
        request: Some(cancel_record("wreq_cancel", &validated, &cancelled)),
    });

    let recovered = recover_owner(&owner(), vec![replay]).unwrap();
    assert_eq!(recovered.runs, vec![cancelled]);
    assert!(
        recovered
            .requests
            .records()
            .iter()
            .any(|record| record.operation == "cancel")
    );
}

#[test]
fn duplicate_owner_request_id_with_conflicting_response_fails() {
    let (first, _) = replay("wrun_one", "wreq_shared");
    let (second, _) = replay("wrun_two", "wreq_shared");
    assert_eq!(
        recover_owner(&owner(), vec![first, second]).map(|_| ()),
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );
}

#[test]
fn recovery_enforces_owner_request_quota() {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    for index in 0..64 {
        replay.commits.push(request_commit(get_record(
            &format!("wreq_get_{index}"),
            &validated,
            Some(validated.clone()),
        )));
    }
    assert_eq!(
        recover_owner(&owner(), vec![replay]).map(|_| ()),
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );
}

fn invalid_get_case(response: Option<WorkflowRunSnapshot>) -> WorkflowJournalReplay {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    replay
        .commits
        .push(request_commit(get_record("wreq_get", &validated, response)));
    replay
}

fn unsupported_request_case() -> WorkflowJournalReplay {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    let mut request = get_record("wreq_get", &validated, Some(validated.clone()));
    request.operation = "wait".into();
    replay.commits.push(request_commit(request));
    replay
}

fn mixed_commit_case() -> WorkflowJournalReplay {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    replay.commits.push(WorkflowJournalCommit {
        run_id: validated.id.clone(),
        events: vec![event(&validated, WorkflowEventPayload::RunStarted)],
        request: Some(get_record("wreq_get", &validated, Some(validated.clone()))),
    });
    replay
}

fn assert_invalid<const N: usize>(replays: [WorkflowJournalReplay; N]) {
    for replay in replays {
        assert_eq!(
            recover_owner(&owner(), vec![replay]).map(|_| ()),
            Err(WorkflowCoordinatorError::RecoveryInvalid)
        );
    }
}
