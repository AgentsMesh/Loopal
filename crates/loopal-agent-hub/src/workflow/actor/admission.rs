mod append;

use super::super::journal::StartJournalRecord;
use super::super::start::StartDecision;
use super::super::{WorkflowCoordinatorError, WorkflowOwner};
use super::WorkflowCoordinator;
pub(in crate::workflow) use append::await_append;
use loopal_protocol::{
    WorkflowGetRequest, WorkflowGetResponse, WorkflowRunSnapshot, WorkflowStartRequest,
    WorkflowStartResponse,
};

pub(super) struct CommittedStart {
    pub(super) response: WorkflowStartResponse,
    pub(super) started: Option<WorkflowRunSnapshot>,
}

impl WorkflowCoordinator {
    pub(in crate::workflow) async fn recover_owner(
        &mut self,
        owner: WorkflowOwner,
    ) -> Result<usize, WorkflowCoordinatorError> {
        if self.mode == super::WorkflowCoordinatorMode::Disabled {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if self.state.is_poisoned(&owner) {
            return Err(WorkflowCoordinatorError::OwnerPoisoned);
        }
        if self.state.is_recovered(&owner) {
            return Ok(self.state.owner_run_count(&owner));
        }
        let journal = self.journal.clone();
        let recovery_owner = owner.clone();
        let recovered = tokio::task::spawn_blocking(move || journal.recover(&recovery_owner))
            .await
            .map_err(|_| WorkflowCoordinatorError::JournalUnavailable)??;
        let delivery_intents = recovered
            .delivery_intents
            .iter()
            .filter(|notification| {
                !recovered
                    .acked_deliveries
                    .contains(&notification.delivery_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let count = self.state.install_recovered(owner.clone(), recovered)?;
        for notification in delivery_intents {
            self.terminal_delivery_payloads
                .insert(notification.delivery_id.clone(), notification);
        }
        let seed = self.state.owner_snapshots(&owner);
        for snapshot in seed {
            self.publish_revision(&owner, &snapshot);
        }
        // A zero grace is the deterministic test/default compatibility mode:
        // use a sentinel so recovery does not consume an extra clock sample.
        let recovery_now = if self.recovery_grace_ms == 0 {
            u64::MAX
        } else {
            self.clock.now_unix_ms()
        };
        for run in self.state.owner_snapshots(&owner) {
            for attempt in run
                .attempts
                .into_iter()
                .filter(|attempt| !attempt.state.is_terminal())
            {
                self.recovery_deadlines.insert(
                    attempt.id,
                    recovery_now.saturating_add(self.recovery_grace_ms),
                );
            }
        }
        if self.mode.executes()
            && let Err(error) = self.reconcile_recovered(&owner, recovery_now).await
        {
            self.poison_owner(owner);
            return Err(error);
        }
        Ok(count)
    }

    pub(super) async fn admit_start(
        &mut self,
        owner: WorkflowOwner,
        request: WorkflowStartRequest,
    ) -> Result<CommittedStart, WorkflowCoordinatorError> {
        if self.mode == super::WorkflowCoordinatorMode::Disabled {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if !self.state.is_recovered(&owner) {
            self.recover_owner(owner.clone()).await?;
        }
        self.trusted_ceilings.validate(&request.spec.limits)?;
        let prepared = match self.state.prepare_start(
            owner.clone(),
            request,
            self.clock.as_ref(),
            self.ids.as_ref(),
        )? {
            StartDecision::Replay(response) => {
                return Ok(CommittedStart {
                    response,
                    started: None,
                });
            }
            StartDecision::New(prepared) => prepared,
        };
        let record = StartJournalRecord {
            owner: prepared.owner.clone(),
            planned: prepared.planned.clone(),
            event: prepared.event.clone(),
            request: prepared.request.clone(),
        };
        let journal = self.journal.clone();
        let append = tokio::task::spawn_blocking(move || journal.append_start(record));
        if let Err(error) = await_append(append).await {
            self.poison_owner(owner);
            return Err(error);
        }
        let started = prepared.snapshot.clone();
        let response = self.state.commit_start(*prepared);
        Ok(CommittedStart {
            response,
            started: Some(started),
        })
    }

    pub(super) async fn admit_get(
        &mut self,
        owner: WorkflowOwner,
        request: WorkflowGetRequest,
    ) -> Result<WorkflowGetResponse, WorkflowCoordinatorError> {
        if self.mode == super::WorkflowCoordinatorMode::Disabled {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if !self.state.is_recovered(&owner) {
            self.recover_owner(owner.clone()).await?;
        }
        let prepared = match self.state.prepare_get(owner.clone(), request)? {
            super::super::get::GetDecision::Replay(response) => return Ok(*response),
            super::super::get::GetDecision::New(prepared) => prepared,
        };
        if prepared.journaled {
            let journal = self.journal.clone();
            let journal_owner = prepared.owner.clone();
            let run_id = prepared
                .response
                .run
                .as_ref()
                .expect("journaled owned run")
                .id
                .clone();
            let record = prepared.record.clone();
            let append = tokio::task::spawn_blocking(move || {
                journal.append_request(&journal_owner, &run_id, record)
            });
            if let Err(error) = await_append(append).await {
                self.poison_owner(owner);
                return Err(error);
            }
        }
        Ok(self.state.commit_get(*prepared))
    }

    pub(super) async fn snapshot(
        &mut self,
        owner: WorkflowOwner,
    ) -> Result<loopal_protocol::WorkflowRunsSnapshot, WorkflowCoordinatorError> {
        if self.mode == super::WorkflowCoordinatorMode::Disabled {
            return Err(WorkflowCoordinatorError::Disabled);
        }
        if !owner.is_valid() {
            return Err(WorkflowCoordinatorError::InvalidOwner);
        }
        if self.state.is_poisoned(&owner) {
            return Err(WorkflowCoordinatorError::OwnerPoisoned);
        }
        if !self.state.is_recovered(&owner) {
            self.recover_owner(owner.clone()).await?;
        }
        Ok(self.state.owner_workflow_snapshot(&owner))
    }
}
