use loopal_protocol::{
    WorkflowEventPayload, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome,
};

use super::{recover_owner, support::*};

#[test]
fn terminal_delivery_ack_recovers_as_owner_fact() {
    let (mut replay, validated) = replay("wrun_one", "wreq_start");
    let cancel = event(
        &validated,
        WorkflowEventPayload::CancelRequested { reason: None },
    );
    let cancelled = super::super::apply_event(&validated, &cancel).unwrap();
    replay.commits.push(event_commit(&validated.id, cancel));
    let delivery_id =
        WorkflowTerminalDeliveryId::new("session", cancelled.id.clone(), cancelled.revision);
    replay.delivery_intents.push(WorkflowTerminalNotification {
        delivery_id: delivery_id.clone(),
        state: WorkflowRunState::Cancelled,
        run_goal: cancelled.spec.run_goal.clone(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "cancelled".into(),
    });
    replay.delivery_acks.push(delivery_id.clone());

    let recovered = recover_owner(&owner(), vec![replay]).unwrap();
    assert_eq!(recovered.delivery_intents.len(), 1);
    assert_eq!(recovered.acked_deliveries.len(), 1);
    assert!(recovered.acked_deliveries.contains(&delivery_id));
}

#[test]
fn delivery_ack_requires_exact_terminal_snapshot_identity() {
    let (mut active, validated) = replay("wrun_one", "wreq_start");
    active.delivery_acks.push(WorkflowTerminalDeliveryId::new(
        "session",
        validated.id.clone(),
        validated.revision,
    ));

    let (mut wrong_revision, validated) = replay("wrun_two", "wreq_second");
    let cancel = event(
        &validated,
        WorkflowEventPayload::CancelRequested { reason: None },
    );
    wrong_revision
        .commits
        .push(event_commit(&validated.id, cancel));
    wrong_revision
        .delivery_acks
        .push(WorkflowTerminalDeliveryId::new(
            "session",
            validated.id,
            validated.revision,
        ));

    for replay in [active, wrong_revision] {
        assert_eq!(
            recover_owner(&owner(), vec![replay]).map(|_| ()),
            Err(super::super::WorkflowCoordinatorError::RecoveryInvalid)
        );
    }
}

#[test]
fn raw_delivery_ack_rejects_wrong_session_and_run_before_revision_checks() {
    for mutate in [
        |id: &mut WorkflowTerminalDeliveryId| id.session_id = "other".into(),
        |id: &mut WorkflowTerminalDeliveryId| id.run_id = "wrun_other".into(),
    ] {
        let (mut replay, validated) = replay("wrun_ack_identity", "wreq_start");
        let cancel = event(
            &validated,
            WorkflowEventPayload::CancelRequested { reason: None },
        );
        let cancelled = super::super::apply_event(&validated, &cancel).unwrap();
        replay.commits.push(event_commit(&validated.id, cancel));
        let mut delivery_id =
            WorkflowTerminalDeliveryId::new("session", cancelled.id, cancelled.revision);
        mutate(&mut delivery_id);
        replay.delivery_acks.push(delivery_id);

        assert_eq!(
            recover_owner(&owner(), vec![replay]).map(|_| ()),
            Err(super::super::WorkflowCoordinatorError::RecoveryInvalid)
        );
    }
}

#[test]
fn raw_delivery_intent_validates_each_snapshot_identity_component() {
    type Mutator = fn(&mut WorkflowTerminalNotification);
    let mutators: [Mutator; 5] = [
        |notification| notification.delivery_id.session_id = "other".into(),
        |notification| notification.delivery_id.run_id = "wrun_other".into(),
        |notification| notification.delivery_id.terminal_revision += 1,
        |notification| {
            notification.state = WorkflowRunState::Failed;
            notification.outcome = WorkflowTerminalOutcome::Failed {
                class: loopal_protocol::WorkflowFailureClass::Permanent,
                reason: "failed".into(),
            };
        },
        |notification| {
            notification.content =
                "x".repeat(loopal_protocol::MAX_WORKFLOW_TERMINAL_CONTENT_BYTES + 1);
        },
    ];
    for (index, mutate) in mutators.into_iter().enumerate() {
        let (mut replay, validated) = replay(&format!("wrun_intent_{index}"), "wreq_start");
        let cancel = event(
            &validated,
            WorkflowEventPayload::CancelRequested { reason: None },
        );
        let cancelled = super::super::apply_event(&validated, &cancel).unwrap();
        replay.commits.push(event_commit(&validated.id, cancel));
        let mut notification = WorkflowTerminalNotification {
            delivery_id: WorkflowTerminalDeliveryId::new(
                "session",
                cancelled.id.clone(),
                cancelled.revision,
            ),
            state: WorkflowRunState::Cancelled,
            run_goal: cancelled.spec.run_goal,
            outcome: WorkflowTerminalOutcome::Cancelled {
                reason: "cancelled".into(),
            },
            content: "cancelled".into(),
        };
        mutate(&mut notification);
        replay.delivery_intents.push(notification);

        assert_eq!(
            recover_owner(&owner(), vec![replay]).map(|_| ()),
            Err(super::super::WorkflowCoordinatorError::RecoveryInvalid)
        );
    }
}
