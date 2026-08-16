use loopal_protocol::{
    QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowEvent, WorkflowEventPayload,
    WorkflowGetRequest, WorkflowGetResponse, WorkflowLimits, WorkflowOutputContract,
    WorkflowRequestId, WorkflowRequestRecord, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowRunSummary, WorkflowSpec, WorkflowStartRequest, WorkflowStartResponse,
    WorkflowWorkerProfileRef,
};
use loopal_storage::{WorkflowJournalCommit, WorkflowJournalInit, WorkflowJournalReplay};

use super::super::{WorkflowOwner, apply_event};

pub(super) fn replay(
    run_id: &str,
    request_id: &str,
) -> (WorkflowJournalReplay, WorkflowRunSnapshot) {
    let planned = WorkflowRunSnapshot::planned(
        WorkflowRunId::new(run_id),
        QualifiedAddress::local("root"),
        spec(),
        10,
    );
    let validated_event = event(&planned, WorkflowEventPayload::SpecValidated);
    let validated = apply_event(&planned, &validated_event).unwrap();
    let start = WorkflowStartRequest {
        request_id: request_id.into(),
        spec: planned.spec.clone(),
    };
    let record = WorkflowRequestRecord {
        request_id: start.request_id.clone(),
        operation: "start".into(),
        payload: serde_json::to_value(start).unwrap(),
        response: serde_json::to_value(WorkflowStartResponse {
            summary: WorkflowRunSummary::from(&validated),
        })
        .unwrap(),
    };
    (
        WorkflowJournalReplay {
            init: Some(WorkflowJournalInit {
                snapshot: planned,
                events: vec![validated_event],
                request: Some(record),
            }),
            ..Default::default()
        },
        validated,
    )
}

pub(super) fn get_record(
    request_id: &str,
    snapshot: &WorkflowRunSnapshot,
    run: Option<WorkflowRunSnapshot>,
) -> WorkflowRequestRecord {
    let request = WorkflowGetRequest {
        request_id: WorkflowRequestId::new(request_id),
        run_id: snapshot.id.clone(),
    };
    WorkflowRequestRecord {
        request_id: request.request_id.clone(),
        operation: "get".into(),
        payload: serde_json::to_value(request).unwrap(),
        response: serde_json::to_value(WorkflowGetResponse { run }).unwrap(),
    }
}

pub(super) fn cancel_record(
    request_id: &str,
    before: &WorkflowRunSnapshot,
    after: &WorkflowRunSnapshot,
) -> WorkflowRequestRecord {
    let request = loopal_protocol::WorkflowCancelRequest {
        request_id: WorkflowRequestId::new(request_id),
        run_id: before.id.clone(),
        reason: Some("stop".into()),
    };
    WorkflowRequestRecord {
        request_id: request.request_id.clone(),
        operation: "cancel".into(),
        payload: serde_json::to_value(request).unwrap(),
        response: serde_json::to_value(loopal_protocol::WorkflowCancelResponse {
            summary: WorkflowRunSummary::from(after),
            already_terminal: false,
        })
        .unwrap(),
    }
}

pub(super) fn event(run: &WorkflowRunSnapshot, payload: WorkflowEventPayload) -> WorkflowEvent {
    WorkflowEvent {
        run_id: run.id.clone(),
        revision: run.revision + 1,
        occurred_at_unix_ms: run.updated_at_unix_ms + 1,
        payload,
    }
}

pub(super) fn request_commit(request: WorkflowRequestRecord) -> WorkflowJournalCommit {
    let run_id: WorkflowGetRequest = serde_json::from_value(request.payload.clone()).unwrap();
    WorkflowJournalCommit {
        run_id: run_id.run_id,
        events: Vec::new(),
        request: Some(request),
    }
}

pub(super) fn event_commit(run_id: &WorkflowRunId, event: WorkflowEvent) -> WorkflowJournalCommit {
    WorkflowJournalCommit {
        run_id: run_id.clone(),
        events: vec![event],
        request: None,
    }
}

pub(super) fn owner() -> WorkflowOwner {
    WorkflowOwner::new("session", QualifiedAddress::local("root"))
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "recover".into(),
        nodes: vec![node("source", &[]), node("output", &["source"])],
        limits: WorkflowLimits {
            max_nodes: 8,
            max_parallel: 2,
            max_attempts: 8,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: "output".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}

fn node(id: &str, dependencies: &[&str]) -> WorkflowAgentNode {
    WorkflowAgentNode {
        id: id.into(),
        dependencies: dependencies.iter().copied().map(Into::into).collect(),
        task: format!("complete {id}"),
        worker_profile: WorkflowWorkerProfileRef::new("default"),
    }
}
