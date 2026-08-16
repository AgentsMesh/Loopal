use std::path::Path;

use loopal_protocol::{
    QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowEvent, WorkflowEventPayload,
    WorkflowJsonValidator, WorkflowLimits, WorkflowOutputContract, WorkflowRunId,
    WorkflowRunSnapshot, WorkflowRunState, WorkflowSpec, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome, WorkflowWorkerProfileRef,
};

use super::{ReplayJsonValidator, current, validate_terminal_intent};
use crate::workflow_journal::error::WorkflowJournalError;
use crate::workflow_journal::record::WorkflowJournalInit;
use crate::workflow_journal::replay::WorkflowJournalReplay;

fn snapshot() -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_snapshot_test"),
        QualifiedAddress::local("root"),
        WorkflowSpec {
            version: WORKFLOW_SPEC_V1,
            run_goal: "validate terminal replay".into(),
            nodes: vec![WorkflowAgentNode {
                id: "output".into(),
                dependencies: Vec::new(),
                task: "produce output".into(),
                worker_profile: WorkflowWorkerProfileRef::new("default"),
            }],
            limits: WorkflowLimits {
                max_nodes: 1,
                max_parallel: 1,
                max_attempts: 1,
                run_deadline_ms: 60_000,
                attempt_timeout_ms: 30_000,
                max_output_bytes: 1_024,
            },
            output_node: "output".into(),
            output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
        },
        1,
    )
}

fn replay(snapshot: WorkflowRunSnapshot, events: Vec<WorkflowEvent>) -> WorkflowJournalReplay {
    WorkflowJournalReplay {
        init: Some(WorkflowJournalInit {
            snapshot,
            events,
            request: None,
        }),
        ..Default::default()
    }
}

fn event(revision: u64, payload: WorkflowEventPayload) -> WorkflowEvent {
    WorkflowEvent {
        run_id: WorkflowRunId::new("wrun_snapshot_test"),
        revision,
        occurred_at_unix_ms: 2,
        payload,
    }
}

fn notification(revision: u64) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            "session-snapshot",
            WorkflowRunId::new("wrun_snapshot_test"),
            revision,
        ),
        state: WorkflowRunState::Cancelled,
        run_goal: "validate terminal replay".into(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "Workflow cancelled.".into(),
    }
}

fn corruption_detail(error: WorkflowJournalError) -> String {
    match error {
        WorkflowJournalError::Corruption { detail, .. } => detail,
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn current_requires_init_and_rejects_stale_or_illegal_replay_events() {
    let path = Path::new("snapshot.jsonl");
    let error = current(&WorkflowJournalReplay::default(), path, 3).unwrap_err();
    assert!(corruption_detail(error).contains("initialized journal"));

    let stale = replay(
        snapshot(),
        vec![event(0, WorkflowEventPayload::SpecValidated)],
    );
    let error = current(&stale, path, 4).unwrap_err();
    assert!(corruption_detail(error).contains("stale event"));

    let illegal = replay(snapshot(), vec![event(1, WorkflowEventPayload::RunStarted)]);
    let error = current(&illegal, path, 5).unwrap_err();
    assert!(corruption_detail(error).contains("snapshot replay failed"));
}

#[test]
fn terminal_intent_revision_is_bound_to_replayed_snapshot() {
    let path = Path::new("snapshot.jsonl");
    let mut terminal = snapshot();
    terminal.state = WorkflowRunState::Cancelled;
    terminal.revision = 2;
    let replay = replay(terminal, Vec::new());

    let error = validate_terminal_intent(path, &replay, &notification(1), 9).unwrap_err();
    assert!(corruption_detail(error).contains("snapshot revision mismatch"));
    validate_terminal_intent(path, &replay, &notification(2), 10).unwrap();
}

#[test]
fn replay_json_validator_accepts_protocol_json_values() {
    WorkflowJsonValidator::validate(
        &ReplayJsonValidator,
        &serde_json::json!({"type": "object"}),
        &serde_json::json!({"answer": 42}),
    )
    .unwrap();
}
