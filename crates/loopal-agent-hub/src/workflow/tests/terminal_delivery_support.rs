use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use loopal_protocol::{
    WorkflowRunSnapshot, WorkflowTerminalDisposition, WorkflowTerminalNotification,
};
use tokio::sync::Notify;

use super::super::actor::{WorkflowRuntimeConfig, WorkflowTrustedCeilings};
use super::super::recovery::RecoveredOwner;
use super::super::scheduler::UnavailableWorkflowSpawner;
use super::super::terminal_delivery::WorkflowTerminalSink;
use super::super::{WorkflowCoordinator, WorkflowCoordinatorHandle, WorkflowCoordinatorMode};
use super::journal_support::TestJournal;
use super::support::{TestClock, TestIds};

pub(super) struct TestTerminalSink {
    results: Mutex<VecDeque<Result<WorkflowTerminalDisposition, String>>>,
    panics: Mutex<usize>,
    deliveries: Mutex<Vec<WorkflowTerminalNotification>>,
    delivered: Notify,
}

impl TestTerminalSink {
    pub(super) fn new(
        results: impl IntoIterator<Item = Result<WorkflowTerminalDisposition, String>>,
    ) -> Self {
        Self {
            results: Mutex::new(results.into_iter().collect()),
            panics: Mutex::new(0),
            deliveries: Mutex::new(Vec::new()),
            delivered: Notify::new(),
        }
    }

    pub(super) fn deliveries(&self) -> Vec<WorkflowTerminalNotification> {
        self.deliveries.lock().unwrap().clone()
    }

    pub(super) fn push_panic(&self) {
        *self.panics.lock().unwrap() += 1;
    }

    pub(super) async fn wait_for_deliveries(&self, expected: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let notified = self.delivered.notified();
                if self.deliveries.lock().unwrap().len() >= expected {
                    return;
                }
                notified.await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {expected} terminal deliveries"));
    }
}

#[async_trait::async_trait]
impl WorkflowTerminalSink for TestTerminalSink {
    async fn deliver(
        &self,
        _owner: &super::super::WorkflowOwner,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowTerminalDisposition, String> {
        self.deliveries.lock().unwrap().push(notification);
        self.delivered.notify_waiters();
        let mut panics = self.panics.lock().unwrap();
        if *panics > 0 {
            *panics -= 1;
            panic!("injected terminal sink panic");
        }
        drop(panics);
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(WorkflowTerminalDisposition::Queued))
    }
}

pub(super) fn coordinator(
    run: WorkflowRunSnapshot,
    acked: impl IntoIterator<Item = loopal_protocol::WorkflowTerminalDeliveryId>,
    sink: Arc<TestTerminalSink>,
) -> (
    WorkflowCoordinatorHandle,
    tokio::task::JoinHandle<()>,
    Arc<TestJournal>,
) {
    coordinator_with_seed(
        run,
        acked,
        sink,
        loopal_output_guard::FinalSinkRedactionSeed::new(),
    )
}

pub(super) fn coordinator_with_seed(
    run: WorkflowRunSnapshot,
    acked: impl IntoIterator<Item = loopal_protocol::WorkflowTerminalDeliveryId>,
    sink: Arc<TestTerminalSink>,
    redaction_seed: loopal_output_guard::FinalSinkRedactionSeed,
) -> (
    WorkflowCoordinatorHandle,
    tokio::task::JoinHandle<()>,
    Arc<TestJournal>,
) {
    let journal = Arc::new(TestJournal::new());
    let workflow_owner = super::support::owner("session", "root");
    let intent = super::super::terminal_delivery::payload::from_snapshot(
        &workflow_owner,
        &run,
        &redaction_seed,
    )
    .unwrap();
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![run],
        requests: Default::default(),
        delivery_intents: vec![intent],
        acked_deliveries: acked.into_iter().collect(),
    }));
    let (handle, task) = WorkflowCoordinator::spawn_with_runtime_config_and_sinks(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([])),
        Arc::new(TestIds::new([])),
        journal.clone(),
        Arc::new(UnavailableWorkflowSpawner),
        WorkflowRuntimeConfig {
            ceilings: WorkflowTrustedCeilings::PROTOCOL_MAXIMUM,
            cancel_grace_ms: 1_000,
            recovery_grace_ms: 0,
            redaction_seed,
        },
        super::super::actor::WorkflowCoordinatorSinks {
            event_sink: None,
            terminal_sink: sink,
        },
    );
    (handle, task, journal)
}
