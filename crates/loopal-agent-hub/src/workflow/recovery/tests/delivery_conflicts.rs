use std::collections::HashSet;

use loopal_protocol::{
    WorkflowEventPayload, WorkflowFailureClass, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome,
};

use super::{recover_owner, support::*};
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::recovery::RecoveredOwner;
use crate::workflow::state::WorkflowActorState;

fn terminal_recovered() -> RecoveredOwner {
    let (mut replay, validated) = replay("wrun_delivery", "wreq_start");
    let cancel = event(
        &validated,
        WorkflowEventPayload::CancelRequested { reason: None },
    );
    let cancelled = super::super::apply_event(&validated, &cancel).unwrap();
    replay.commits.push(event_commit(&validated.id, cancel));
    replay.delivery_intents.push(notification(&cancelled));
    recover_owner(&owner(), vec![replay]).unwrap()
}

fn notification(run: &loopal_protocol::WorkflowRunSnapshot) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new("session", run.id.clone(), run.revision),
        state: WorkflowRunState::Cancelled,
        run_goal: run.spec.run_goal.clone(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "cancelled".into(),
    }
}

fn conflict(mutate: impl FnOnce(&mut RecoveredOwner)) {
    let mut recovered = terminal_recovered();
    mutate(&mut recovered);
    assert_eq!(
        WorkflowActorState::new().install_recovered(owner(), recovered),
        Err(WorkflowCoordinatorError::RecoveryConflict)
    );
}

#[test]
fn recovered_delivery_intent_requires_exact_owner_run_revision_and_state() {
    conflict(|recovered| recovered.delivery_intents[0].delivery_id.session_id.clear());
    conflict(|recovered| recovered.delivery_intents[0].delivery_id.session_id = "other".into());
    conflict(|recovered| recovered.delivery_intents[0].delivery_id.run_id = "wrun_other".into());
    conflict(|recovered| recovered.delivery_intents[0].delivery_id.terminal_revision += 1);
    conflict(|recovered| {
        recovered.delivery_intents[0].state = WorkflowRunState::Failed;
        recovered.delivery_intents[0].outcome = WorkflowTerminalOutcome::Failed {
            class: WorkflowFailureClass::Permanent,
            reason: "failed".into(),
        };
    });
    conflict(|recovered| {
        recovered.runs[0].state = WorkflowRunState::Running;
        recovered.delivery_intents[0].state = WorkflowRunState::Running;
    });
}

#[test]
fn recovered_delivery_ids_are_unique_and_acks_require_an_intent() {
    conflict(|recovered| {
        recovered
            .delivery_intents
            .push(recovered.delivery_intents[0].clone());
    });
    conflict(|recovered| {
        recovered.acked_deliveries = HashSet::from([WorkflowTerminalDeliveryId::new(
            "session",
            "wrun_missing".into(),
            1,
        )]);
    });
}
