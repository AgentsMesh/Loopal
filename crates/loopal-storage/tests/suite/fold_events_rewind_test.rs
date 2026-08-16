use chrono::Utc;
use loopal_storage::fold_events;
use loopal_turn::{TurnEvent, TurnId, TurnOutcome, TurnTrigger};

#[test]
fn rewind_covers_cancel_and_truncation_boundaries() {
    let finished = TurnId::from_string("finished");
    let active = TurnId::from_string("active");
    let missing = TurnId::from_string("missing");
    let now = Utc::now();
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: finished.clone(),
            started_at: now,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::TurnEnded {
            turn_id: finished.clone(),
            outcome: TurnOutcome::Complete,
        },
        TurnEvent::Rewound {
            at: now,
            keep: 1,
            cancel_in_progress: Some(finished.clone()),
        },
        TurnEvent::TurnStarted {
            turn_id: active.clone(),
            started_at: now,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::Rewound {
            at: now,
            keep: 1,
            cancel_in_progress: Some(active),
        },
        TurnEvent::Rewound {
            at: now,
            keep: 1,
            cancel_in_progress: Some(missing),
        },
        TurnEvent::Rewound {
            at: now,
            keep: 1,
            cancel_in_progress: None,
        },
    ];

    let turns = fold_events(events);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].id, finished);
    assert_eq!(turns[0].outcome, TurnOutcome::Complete);
}

#[test]
fn clear_cancels_the_active_turn_before_removal() {
    let turn_id = TurnId::from_string("active");
    let now = Utc::now();
    let turns = fold_events(vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: now,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::Cleared {
            at: now,
            cancel_in_progress: Some(turn_id),
        },
    ]);

    assert!(turns.is_empty());
}
