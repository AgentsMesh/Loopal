use loopal_protocol::{
    WorkflowRunState, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
    WorkflowTerminalOutcome,
};

use super::{WorkflowJournalAppendKind, WorkflowJournalReplay, validate};

fn delivery_id(run: &str) -> WorkflowTerminalDeliveryId {
    WorkflowTerminalDeliveryId::new("session", run.into(), 0)
}

fn notification(run: &str) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: delivery_id(run),
        state: WorkflowRunState::Cancelled,
        run_goal: "goal".into(),
        outcome: WorkflowTerminalOutcome::Cancelled {
            reason: "cancelled".into(),
        },
        content: "cancelled".into(),
    }
}

#[test]
fn commit_rejects_a_terminal_ack_even_without_an_intent() {
    let replay = WorkflowJournalReplay {
        delivery_acks: vec![delivery_id("wrun_one")],
        ..Default::default()
    };

    assert!(
        validate(
            std::path::Path::new("journal.jsonl"),
            &replay,
            &WorkflowJournalAppendKind::Commit,
            None,
        )
        .is_err()
    );
}

#[test]
fn second_distinct_terminal_intent_is_rejected() {
    let replay = WorkflowJournalReplay {
        delivery_intents: vec![notification("wrun_one")],
        ..Default::default()
    };

    assert!(
        validate(
            std::path::Path::new("journal.jsonl"),
            &replay,
            &WorkflowJournalAppendKind::DeliveryIntent(notification("wrun_two")),
            None,
        )
        .is_err()
    );
}
