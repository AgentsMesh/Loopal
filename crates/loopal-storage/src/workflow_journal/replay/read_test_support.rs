use loopal_protocol::{
    QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowLimits, WorkflowOutputContract,
    WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState, WorkflowSpec, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome, WorkflowWorkerProfileRef,
};

use crate::workflow_journal::error::WorkflowJournalError;
use crate::workflow_journal::record::{WorkflowJournalEntry, WorkflowJournalInit};
use crate::workflow_journal::replay::WorkflowJournalReplay;

pub(super) fn snapshot() -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_read_test"),
        QualifiedAddress::local("root"),
        WorkflowSpec {
            version: WORKFLOW_SPEC_V1,
            run_goal: "test replay apply".into(),
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

pub(super) fn init_entry() -> WorkflowJournalEntry {
    WorkflowJournalEntry::init(snapshot(), Vec::new(), None)
}

pub(super) fn terminal_replay() -> WorkflowJournalReplay {
    let mut snapshot = snapshot();
    snapshot.state = WorkflowRunState::Cancelled;
    snapshot.revision = 2;
    WorkflowJournalReplay {
        init: Some(WorkflowJournalInit {
            snapshot,
            events: Vec::new(),
            request: None,
        }),
        ..Default::default()
    }
}

pub(super) fn delivery_id() -> WorkflowTerminalDeliveryId {
    WorkflowTerminalDeliveryId::new("session-read", WorkflowRunId::new("wrun_read_test"), 2)
}

pub(super) fn notification() -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: delivery_id(),
        state: WorkflowRunState::Cancelled,
        run_goal: "test replay apply".into(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "Workflow cancelled.".into(),
    }
}

pub(super) fn assert_corrupt(result: Result<(), WorkflowJournalError>, detail: &str) {
    match result.unwrap_err() {
        WorkflowJournalError::Corruption { detail: actual, .. } => {
            assert!(actual.contains(detail), "{actual:?}")
        }
        other => panic!("unexpected error: {other}"),
    }
}
