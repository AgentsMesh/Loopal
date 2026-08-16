use loopal_protocol::{QualifiedAddress, WorkflowEventPayload, WorkflowRunId};
use loopal_storage::WorkflowJournalCommit;
use serde_json::json;

use super::support::replay as make_replay;
use super::{WorkflowCoordinatorError, recover_owner, support::*};

fn invalid(replay: loopal_storage::WorkflowJournalReplay) {
    assert_eq!(
        recover_owner(&owner(), vec![replay]).map(|_| ()),
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );
}

#[test]
fn recovery_rejects_malformed_init_and_commit_shapes() {
    invalid(Default::default());

    let (mut case_replay, _) = make_replay("wrun_root", "wreq_root");
    case_replay.init.as_mut().unwrap().snapshot.root_agent = QualifiedAddress::local("other");
    invalid(case_replay);

    let (mut replay, _) = make_replay("wrun_events", "wreq_events");
    let duplicate_event = replay.init.as_ref().unwrap().events[0].clone();
    replay.init.as_mut().unwrap().events.push(duplicate_event);
    invalid(replay);

    let (mut replay, _) = make_replay("wrun_payload", "wreq_payload");
    replay.init.as_mut().unwrap().events[0].payload = WorkflowEventPayload::RunStarted;
    invalid(replay);

    let (mut replay, _) = make_replay("wrun_request", "wreq_request");
    replay.init.as_mut().unwrap().request = None;
    invalid(replay);

    let (mut replay, validated) = make_replay("wrun_commit", "wreq_commit");
    replay.commits.push(WorkflowJournalCommit {
        run_id: WorkflowRunId::new("wrun_other"),
        events: Vec::new(),
        request: Some(get_record("wreq_get", &validated, Some(validated.clone()))),
    });
    invalid(replay);

    let (mut replay, validated) = make_replay("wrun_empty", "wreq_empty");
    replay.commits.push(WorkflowJournalCommit {
        run_id: validated.id,
        events: Vec::new(),
        request: None,
    });
    invalid(replay);
}

#[test]
fn start_and_get_records_bind_every_request_and_response_field() {
    let mutators: [fn(&mut loopal_protocol::WorkflowRequestRecord); 6] = [
        |record: &mut loopal_protocol::WorkflowRequestRecord| record.operation = "get".into(),
        |record| record.payload = json!({}),
        |record| record.response = json!({}),
        |record| record.payload["request_id"] = json!("wreq_other"),
        |record| record.payload["spec"]["run_goal"] = json!("different"),
        |record| record.response["summary"]["revision"] = json!(99),
    ];
    for mutate in mutators {
        let (mut replay, _) = make_replay("wrun_start", "wreq_start");
        mutate(replay.init.as_mut().unwrap().request.as_mut().unwrap());
        invalid(replay);
    }

    let mutators: [fn(&mut loopal_protocol::WorkflowRequestRecord); 4] = [
        |record: &mut loopal_protocol::WorkflowRequestRecord| record.payload = json!({}),
        |record| record.response = json!({}),
        |record| record.payload["request_id"] = json!("wreq_other"),
        |record| record.payload["run_id"] = json!("wrun_other"),
    ];
    for mutate in mutators {
        let (mut replay, validated) = make_replay("wrun_get", "wreq_start");
        let mut record = get_record("wreq_get", &validated, Some(validated.clone()));
        mutate(&mut record);
        replay.commits.push(WorkflowJournalCommit {
            run_id: validated.id,
            events: Vec::new(),
            request: Some(record),
        });
        invalid(replay);
    }
}

#[test]
fn request_only_cancel_validates_all_bound_fields() {
    let (mut valid, snapshot) = make_replay("wrun_cancel", "wreq_start");
    valid.commits.push(WorkflowJournalCommit {
        run_id: snapshot.id.clone(),
        events: Vec::new(),
        request: Some(cancel_record("wreq_cancel", &snapshot, &snapshot)),
    });
    assert!(recover_owner(&owner(), vec![valid]).is_ok());

    let mutators: [fn(&mut loopal_protocol::WorkflowRequestRecord); 6] = [
        |record: &mut loopal_protocol::WorkflowRequestRecord| record.payload = json!({}),
        |record| record.response = json!({}),
        |record| record.payload["request_id"] = json!("wreq_other"),
        |record| record.payload["run_id"] = json!("wrun_other"),
        |record| record.response["summary"]["revision"] = json!(99),
        |record| record.response["already_terminal"] = json!(true),
    ];
    for mutate in mutators {
        let (mut replay, current) = make_replay("wrun_cancel_bad", "wreq_start");
        let mut record = cancel_record("wreq_cancel", &current, &current);
        mutate(&mut record);
        replay.commits.push(WorkflowJournalCommit {
            run_id: current.id,
            events: Vec::new(),
            request: Some(record),
        });
        invalid(replay);
    }
}

#[test]
fn duplicate_runs_and_delivery_cardinality_fail_closed() {
    let (same, _) = make_replay("wrun_duplicate", "wreq_start");
    assert_eq!(
        recover_owner(&owner(), vec![same.clone(), same]).map(|_| ()),
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );

    let (mut replay, validated) = make_replay("wrun_ack", "wreq_start");
    let cancel = event(
        &validated,
        WorkflowEventPayload::CancelRequested { reason: None },
    );
    let cancelled = super::super::apply_event(&validated, &cancel).unwrap();
    replay.commits.push(event_commit(&validated.id, cancel));
    replay
        .delivery_acks
        .push(loopal_protocol::WorkflowTerminalDeliveryId::new(
            "session",
            cancelled.id,
            cancelled.revision,
        ));
    invalid(replay);
}
