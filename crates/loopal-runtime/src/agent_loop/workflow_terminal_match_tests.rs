use chrono::Utc;
use loopal_protocol::{
    WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
    WorkflowTerminalOutcome,
};
use loopal_turn::{Turn, TurnEvent, TurnOutcome, TurnTrigger};

use super::*;

fn notification() -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            "session-match",
            WorkflowRunId::new("wrun_match"),
            1,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "goal".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "done".into(),
        },
        content: "done".into(),
    }
}

fn turn(notification: &WorkflowTerminalNotification, outcome: TurnOutcome) -> Turn {
    Turn {
        id: loopal_turn::TurnId::from_string("turn-match"),
        started_at: Utc::now(),
        trigger: TurnTrigger::WorkflowResult {
            session_id: notification.delivery_id.session_id.clone(),
            run_id: notification.delivery_id.run_id.to_string(),
            terminal_revision: notification.delivery_id.terminal_revision,
            payload_digest: notification.payload_digest(),
            state: "succeeded".into(),
            content: notification.content.clone(),
        },
        body: Default::default(),
        outcome,
        last_step_at: None,
    }
}

fn started(turn: &Turn) -> TurnEvent {
    TurnEvent::TurnStarted {
        turn_id: turn.id.clone(),
        started_at: turn.started_at,
        trigger: turn.trigger.clone(),
    }
}

fn edited_identity(
    notification: &WorkflowTerminalNotification,
    outcome: TurnOutcome,
    edit: impl FnOnce(&mut String, &mut String, &mut u64, &mut String),
) -> Turn {
    let mut turn = turn(notification, outcome);
    let TurnTrigger::WorkflowResult {
        session_id,
        run_id,
        terminal_revision,
        payload_digest,
        ..
    } = &mut turn.trigger
    else {
        unreachable!();
    };
    edit(session_id, run_id, terminal_revision, payload_digest);
    turn
}

#[test]
fn exact_completed_delivery_is_not_resumable() {
    let notification = notification();
    let turn = turn(
        &notification,
        TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::CrashRecovery,
        },
    );
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn.id.clone(),
            started_at: turn.started_at,
            trigger: turn.trigger.clone(),
        },
        TurnEvent::TurnEnded {
            turn_id: turn.id.clone(),
            outcome: TurnOutcome::Complete,
        },
    ];
    assert!(matches!(
        classify(
            &events,
            &[turn],
            &notification.delivery_id,
            &notification.payload_digest()
        ),
        PersistedDelivery::Exact {
            should_execute: false
        }
    ));
}

#[test]
fn crash_recovery_resumes_exact_workflow_trigger() {
    let notification = notification();
    let turn = turn(
        &notification,
        TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::CrashRecovery,
        },
    );
    assert!(matches!(
        resume_trigger(std::slice::from_ref(&turn)),
        Some(TurnTrigger::WorkflowResult { .. })
    ));
}

#[test]
fn classify_distinguishes_absent_conflict_and_recoverable_unfinished() {
    let notification = notification();
    let turn = turn(
        &notification,
        TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::CrashRecovery,
        },
    );
    let digest = notification.payload_digest();
    assert!(matches!(
        classify(&[], &[], &notification.delivery_id, &digest),
        PersistedDelivery::Absent
    ));
    assert!(matches!(
        classify(
            &[started(&turn)],
            std::slice::from_ref(&turn),
            &notification.delivery_id,
            "different-payload"
        ),
        PersistedDelivery::Conflict
    ));
    assert!(matches!(
        classify(
            &[started(&turn)],
            std::slice::from_ref(&turn),
            &notification.delivery_id,
            &digest
        ),
        PersistedDelivery::Exact {
            should_execute: true
        }
    ));
}

#[test]
fn delivery_identity_requires_every_component() {
    let notification = notification();
    let outcome = TurnOutcome::Complete;
    let variants = [
        edited_identity(&notification, outcome.clone(), |session, _, _, _| {
            *session = "other-session".into();
        }),
        edited_identity(&notification, outcome.clone(), |_, run, _, _| {
            *run = "wrun_other".into();
        }),
        edited_identity(&notification, outcome, |_, _, revision, _| {
            *revision += 1;
        }),
    ];
    assert!(contains_exact(
        &[turn(&notification, TurnOutcome::Complete)],
        &notification.delivery_id,
        &notification.payload_digest()
    ));
    for variant in variants {
        assert!(!contains_exact(
            &[variant],
            &notification.delivery_id,
            &notification.payload_digest()
        ));
    }
}

#[path = "workflow_terminal_match_tests/resume.rs"]
mod resume;
