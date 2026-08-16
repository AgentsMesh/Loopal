use std::collections::{HashMap, HashSet};

use loopal_protocol::{
    WorkflowGetResponse, WorkflowRequestLedger, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowRunSummary, WorkflowRunsSnapshot, WorkflowStartResponse, WorkflowTerminalDeliveryId,
};

use super::get::PreparedGet;
use super::recovery::RecoveredOwner;
use super::start::PreparedStart;
use super::{WorkflowCoordinatorError, WorkflowOwner};

#[path = "state/delivery.rs"]
mod delivery;

pub(super) struct WorkflowActorState {
    pub(super) runs: HashMap<WorkflowRunId, OwnedRun>,
    pub(super) requests: HashMap<WorkflowOwner, WorkflowRequestLedger>,
    acked_deliveries: HashSet<WorkflowTerminalDeliveryId>,
    recovered_owners: HashSet<WorkflowOwner>,
    poisoned_owners: HashSet<WorkflowOwner>,
}

pub(super) struct OwnedRun {
    owner: WorkflowOwner,
    snapshot: WorkflowRunSnapshot,
}

impl WorkflowActorState {
    pub(super) fn new() -> Self {
        Self {
            runs: HashMap::new(),
            requests: HashMap::new(),
            acked_deliveries: HashSet::new(),
            recovered_owners: HashSet::new(),
            poisoned_owners: HashSet::new(),
        }
    }

    pub(super) fn is_recovered(&self, owner: &WorkflowOwner) -> bool {
        self.recovered_owners.contains(owner)
    }

    pub(super) fn is_poisoned(&self, owner: &WorkflowOwner) -> bool {
        self.poisoned_owners.contains(owner)
    }

    pub(super) fn poison(&mut self, owner: WorkflowOwner) {
        self.poisoned_owners.insert(owner);
    }

    pub(super) fn commit_start(&mut self, prepared: PreparedStart) -> WorkflowStartResponse {
        let run_id = prepared.snapshot.id.clone();
        self.requests
            .insert(prepared.owner.clone(), prepared.next_ledger);
        self.runs.insert(
            run_id,
            OwnedRun {
                owner: prepared.owner,
                snapshot: prepared.snapshot,
            },
        );
        prepared.response
    }

    pub(super) fn install_recovered(
        &mut self,
        owner: WorkflowOwner,
        recovered: RecoveredOwner,
    ) -> Result<usize, WorkflowCoordinatorError> {
        if self.recovered_owners.contains(&owner) {
            return Ok(self.owner_run_count(&owner));
        }
        if self.requests.contains_key(&owner)
            || self.runs.values().any(|run| run.owner == owner)
            || recovered
                .runs
                .iter()
                .any(|run| self.runs.contains_key(&run.id) || run.root_agent != owner.root_agent)
        {
            return Err(WorkflowCoordinatorError::RecoveryConflict);
        }
        let count = recovered.runs.len();
        delivery::validate(&owner, &recovered)?;
        for snapshot in recovered.runs {
            self.runs.insert(
                snapshot.id.clone(),
                OwnedRun {
                    owner: owner.clone(),
                    snapshot,
                },
            );
        }
        self.requests.insert(owner.clone(), recovered.requests);
        self.acked_deliveries.extend(recovered.acked_deliveries);
        self.recovered_owners.insert(owner);
        Ok(count)
    }

    pub(super) fn is_delivery_acked(&self, delivery_id: &WorkflowTerminalDeliveryId) -> bool {
        self.acked_deliveries.contains(delivery_id)
    }

    pub(super) fn commit_delivery_ack(&mut self, delivery_id: WorkflowTerminalDeliveryId) {
        self.acked_deliveries.insert(delivery_id);
    }

    pub(super) fn commit_get(&mut self, prepared: PreparedGet) -> WorkflowGetResponse {
        self.requests.insert(prepared.owner, prepared.next_ledger);
        prepared.response
    }

    pub(super) fn owned_snapshot(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
    ) -> Option<WorkflowRunSnapshot> {
        self.runs
            .get(run_id)
            .filter(|run| &run.owner == owner)
            .map(|run| run.snapshot.clone())
    }

    pub(super) fn commit_transition(
        &mut self,
        owner: &WorkflowOwner,
        snapshot: WorkflowRunSnapshot,
    ) -> Result<(), WorkflowCoordinatorError> {
        let run = self
            .runs
            .get_mut(&snapshot.id)
            .filter(|run| &run.owner == owner)
            .ok_or(WorkflowCoordinatorError::InvalidRunId)?;
        if snapshot.revision != run.snapshot.revision.saturating_add(1) {
            return Err(WorkflowCoordinatorError::UnexpectedStaleEvent);
        }
        run.snapshot = snapshot;
        Ok(())
    }

    pub(super) fn scheduler_runs(&self) -> Vec<(WorkflowOwner, WorkflowRunSnapshot)> {
        self.runs
            .values()
            .map(|run| (run.owner.clone(), run.snapshot.clone()))
            .collect()
    }

    pub(super) fn owner_snapshots(&self, owner: &WorkflowOwner) -> Vec<WorkflowRunSnapshot> {
        let mut snapshots: Vec<_> = self
            .runs
            .values()
            .filter(|run| &run.owner == owner)
            .map(|run| run.snapshot.clone())
            .collect();
        snapshots.sort_by(|left, right| left.id.cmp(&right.id));
        snapshots
    }

    pub(super) fn owner_run_count(&self, owner: &WorkflowOwner) -> usize {
        self.runs.values().filter(|run| &run.owner == owner).count()
    }

    pub(super) fn owner_workflow_snapshot(&self, owner: &WorkflowOwner) -> WorkflowRunsSnapshot {
        let mut active = Vec::new();
        let mut recent = Vec::new();
        for run in self.runs.values().filter(|run| &run.owner == owner) {
            let summary = WorkflowRunSummary::from(&run.snapshot);
            if summary.state.is_terminal() {
                recent.push(summary);
            } else {
                active.push(summary);
            }
        }
        let sort_updated = |left: &WorkflowRunSummary, right: &WorkflowRunSummary| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        };
        active.sort_by(sort_updated);
        recent.sort_by(sort_updated);
        recent.truncate(loopal_protocol::MAX_RECENT_WORKFLOW_SUMMARIES);
        WorkflowRunsSnapshot { active, recent }
    }
}
