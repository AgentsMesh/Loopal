use std::sync::Arc;

use loopal_protocol::{
    QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptCapability,
    WorkflowAttemptId, WorkflowEventPayload, WorkflowLimits, WorkflowNodeId,
    WorkflowOutputContract, WorkflowPermissionCausation, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowSpec, WorkflowWorkerProfileRef,
};
use tokio::sync::mpsc;

use super::super::super::super::{
    WorkflowCoordinator, WorkflowCoordinatorMode, WorkflowRuntimeConfig, WorkflowTrustedCeilings,
};
use crate::types::AgentExecutionRef;
use crate::workflow::journal::UnavailableWorkflowJournals;
use crate::workflow::recovery::{RecoveredOwner, WorkflowAttemptReconnect};
use crate::workflow::scheduler::{
    ActiveAttempt, ActiveAttemptPhase, AttemptKey, UnavailableWorkflowSpawner,
};
use crate::workflow::state::WorkflowActorState;
use crate::workflow::terminal_delivery::UnavailableWorkflowTerminalSink;
use crate::workflow::transition::apply_payload;
use crate::workflow::{
    SystemWorkflowClock, SystemWorkflowIdSource, WorkflowCoordinatorError, WorkflowOwner,
};

fn recovered_run(
    cancelling: bool,
) -> (
    WorkflowRunSnapshot,
    WorkflowPermissionCausation,
    WorkflowAttemptCapability,
) {
    let node_id = WorkflowNodeId::new("node");
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_handshake_invariant"),
        node_id: node_id.clone(),
        attempt_id: WorkflowAttemptId::new("watt_handshake_invariant"),
    };
    let capability = WorkflowAttemptCapability::parse("33".repeat(32)).unwrap();
    let spec = WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "validate handshake invariants".into(),
        nodes: vec![WorkflowAgentNode {
            id: node_id.clone(),
            dependencies: Vec::new(),
            task: "run".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 1_000,
            attempt_timeout_ms: 500,
            max_output_bytes: 1_024,
        },
        output_node: node_id,
        output_contract: WorkflowOutputContract::Text { max_bytes: 512 },
    };
    let mut run = WorkflowRunSnapshot::planned(
        causation.run_id.clone(),
        QualifiedAddress::local("root"),
        spec,
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
        WorkflowEventPayload::AttemptBound {
            node_id: causation.node_id.clone(),
            attempt_id: causation.attempt_id.clone(),
            agent: QualifiedAddress::local("worker"),
        },
        WorkflowEventPayload::AttemptRunning {
            node_id: causation.node_id.clone(),
            attempt_id: causation.attempt_id.clone(),
        },
    ];
    if cancelling {
        payloads.push(WorkflowEventPayload::AttemptStopRequested {
            node_id: causation.node_id.clone(),
            attempt_id: causation.attempt_id.clone(),
            reason: "test invariant".into(),
        });
    }
    for payload in payloads {
        run = apply_payload(&run, payload, run.updated_at_unix_ms + 1)
            .unwrap()
            .1;
    }
    (run, causation, capability)
}

pub(super) fn fixture(
    cancelling: bool,
) -> (WorkflowCoordinator, WorkflowOwner, WorkflowAttemptReconnect) {
    let (run, causation, capability) = recovered_run(cancelling);
    let owner = WorkflowOwner::new(
        "session-handshake-invariant",
        QualifiedAddress::local("root"),
    );
    let execution = AgentExecutionRef::local("worker", 7);
    let mut state = WorkflowActorState::new();
    state
        .install_recovered(
            owner.clone(),
            RecoveredOwner {
                runs: vec![run],
                requests: Default::default(),
                delivery_intents: Vec::new(),
                acked_deliveries: Default::default(),
            },
        )
        .unwrap();
    let active = ActiveAttempt {
        owner: owner.clone(),
        key: AttemptKey {
            run_id: causation.run_id.clone(),
            node_id: causation.node_id.clone(),
            attempt_id: causation.attempt_id.clone(),
        },
        execution: execution.clone(),
        outcome: None,
        outcome_waiter: None,
        shutdown_waiter: None,
        deadline_unix_ms: 1_000,
        shutdown_after_unix_ms: None,
        phase: ActiveAttemptPhase::Running,
        stop: None,
    };
    let (commands, receiver) = mpsc::channel(1);
    let coordinator = WorkflowCoordinator {
        mode: WorkflowCoordinatorMode::ExecutionHarness,
        clock: Arc::new(SystemWorkflowClock),
        ids: Arc::new(SystemWorkflowIdSource),
        journal: Arc::new(UnavailableWorkflowJournals),
        commands: receiver,
        state,
        spawner: Arc::new(UnavailableWorkflowSpawner),
        active: [(causation.attempt_id.clone(), active)].into(),
        pending: Default::default(),
        callbacks: commands.downgrade(),
        cancel_grace_ms: WorkflowRuntimeConfig::test_default().cancel_grace_ms,
        trusted_ceilings: WorkflowTrustedCeilings::PROTOCOL_MAXIMUM,
        recovery_grace_ms: 0,
        recovery_deadlines: Default::default(),
        recovered_adoptions: Default::default(),
        resumed_owners: Default::default(),
        terminal_deliveries: Default::default(),
        terminal_delivery_payloads: Default::default(),
        terminal_delivery_owners: Default::default(),
        terminal_delivery_failure: None,
        revisions: Default::default(),
        event_sink: None,
        terminal_sink: Arc::new(UnavailableWorkflowTerminalSink),
        redaction_seed: loopal_output_guard::FinalSinkRedactionSeed::new(),
    };
    let request = WorkflowAttemptReconnect {
        causation,
        capability,
        execution,
    };
    (coordinator, owner, request)
}

pub(super) async fn expect_error(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    request: &WorkflowAttemptReconnect,
    expected: WorkflowCoordinatorError,
) {
    assert_eq!(
        super::run(coordinator, owner.clone(), request.clone()).await,
        Err(expected)
    );
}

#[path = "recovery_handshake_tests/cases.rs"]
mod cases;
