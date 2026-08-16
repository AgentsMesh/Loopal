use std::sync::Arc;

use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptCapability, WorkflowAttemptId, WorkflowEventPayload,
    WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId, WorkflowRunSnapshot,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerControl, test_spawner};
use super::support::{TestClock, TestIds, spec};
use crate::types::AgentExecutionRef;
use crate::workflow::actor::{WorkflowRuntimeConfig, WorkflowTrustedCeilings};
use crate::workflow::recovery::{RecoveredOwner, WorkflowAttemptReconnect};
use crate::workflow::transition::apply_payload;
use crate::workflow::{
    WorkflowCoordinator, WorkflowCoordinatorError, WorkflowCoordinatorHandle,
    WorkflowCoordinatorMode, WorkflowOwner,
};

pub(super) fn recovered(
    running: bool,
) -> (
    WorkflowRunSnapshot,
    WorkflowPermissionCausation,
    WorkflowAttemptCapability,
) {
    let capability = WorkflowAttemptCapability::parse("11".repeat(32)).unwrap();
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_reconnect"),
        node_id: WorkflowNodeId::new("output"),
        attempt_id: WorkflowAttemptId::new("watt_reconnect"),
    };
    let mut workflow_spec = spec();
    workflow_spec.nodes.remove(0);
    workflow_spec.nodes[0].dependencies.clear();
    let mut run = WorkflowRunSnapshot::planned(
        causation.run_id.clone(),
        QualifiedAddress::local("root"),
        workflow_spec,
        1,
    );
    let mut payloads = vec![
        WorkflowEventPayload::SpecValidated,
        WorkflowEventPayload::RunStarted,
        WorkflowEventPayload::DispatchIntended {
            node_id: causation.node_id.clone(),
            attempt_id: causation.attempt_id.clone(),
            capability_digest: capability.digest(),
        },
    ];
    if running {
        payloads.extend([
            WorkflowEventPayload::AttemptBound {
                node_id: causation.node_id.clone(),
                attempt_id: causation.attempt_id.clone(),
                agent: QualifiedAddress::local("worker"),
            },
            WorkflowEventPayload::AttemptRunning {
                node_id: causation.node_id.clone(),
                attempt_id: causation.attempt_id.clone(),
            },
        ]);
    }
    for payload in payloads {
        run = apply_payload(&run, payload, run.updated_at_unix_ms + 1)
            .unwrap()
            .1;
    }
    (run, causation, capability)
}

pub(super) fn coordinator(
    journal: Arc<TestJournal>,
    clock: Arc<TestClock>,
    run: WorkflowRunSnapshot,
) -> (
    WorkflowCoordinatorHandle,
    tokio::task::JoinHandle<()>,
    SpawnerControl,
) {
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![run],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (spawner, control) = test_spawner();
    let (handle, task) = WorkflowCoordinator::spawn_with_runtime_config(
        WorkflowCoordinatorMode::ExecutionHarness,
        clock,
        Arc::new(TestIds::new([])),
        journal,
        spawner,
        None,
        WorkflowRuntimeConfig {
            ceilings: WorkflowTrustedCeilings::PROTOCOL_MAXIMUM,
            cancel_grace_ms: 1_000,
            recovery_grace_ms: 100,
            redaction_seed: loopal_output_guard::FinalSinkRedactionSeed::new(),
        },
    );
    (handle, task, control)
}

pub(super) async fn begin_handshake(
    handle: WorkflowCoordinatorHandle,
    owner: WorkflowOwner,
    causation: WorkflowPermissionCausation,
    capability: WorkflowAttemptCapability,
    execution: AgentExecutionRef,
) -> Result<loopal_protocol::WorkflowWorkerHandshakeResponse, WorkflowCoordinatorError> {
    handle
        .worker_handshake(
            owner,
            WorkflowAttemptReconnect {
                causation,
                capability,
                execution,
            },
        )
        .await
}
