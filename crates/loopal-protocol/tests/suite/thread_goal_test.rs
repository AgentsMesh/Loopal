use loopal_protocol::thread_goal::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};

const ALL_STATUSES: &[ThreadGoalStatus] = &[
    ThreadGoalStatus::Active,
    ThreadGoalStatus::Paused,
    ThreadGoalStatus::Complete,
    ThreadGoalStatus::Infeasible,
];

const TERMINAL_STATUSES: &[ThreadGoalStatus] =
    &[ThreadGoalStatus::Complete, ThreadGoalStatus::Infeasible];

const NON_REOPEN_REASONS: &[GoalTransitionReason] = &[
    GoalTransitionReason::UserCreated,
    GoalTransitionReason::ModelCompleted,
    GoalTransitionReason::UserCompleted,
    GoalTransitionReason::UserPaused,
    GoalTransitionReason::UserResumed,
    GoalTransitionReason::UserCleared,
    GoalTransitionReason::ModelInfeasible,
    GoalTransitionReason::UserInfeasible,
];

const REOPEN_REASONS: &[GoalTransitionReason] = &[
    GoalTransitionReason::ModelReopened,
    GoalTransitionReason::UserReopened,
];

const ALL_REASONS: &[GoalTransitionReason] = &[
    GoalTransitionReason::UserCreated,
    GoalTransitionReason::ModelCompleted,
    GoalTransitionReason::UserCompleted,
    GoalTransitionReason::ModelReopened,
    GoalTransitionReason::UserReopened,
    GoalTransitionReason::UserPaused,
    GoalTransitionReason::UserResumed,
    GoalTransitionReason::UserCleared,
    GoalTransitionReason::ModelInfeasible,
    GoalTransitionReason::UserInfeasible,
];

#[test]
fn each_legal_transition_is_accepted() {
    use GoalTransitionReason as R;
    use ThreadGoalStatus as S;
    let cases: &[(ThreadGoalStatus, ThreadGoalStatus, GoalTransitionReason)] = &[
        (S::Active, S::Complete, R::ModelCompleted),
        (S::Active, S::Complete, R::UserCompleted),
        (S::Active, S::Paused, R::UserPaused),
        (S::Active, S::Infeasible, R::ModelInfeasible),
        (S::Active, S::Infeasible, R::UserInfeasible),
        (S::Paused, S::Active, R::UserResumed),
        (S::Paused, S::Complete, R::UserCompleted),
        (S::Paused, S::Infeasible, R::UserInfeasible),
        (S::Complete, S::Active, R::ModelReopened),
        (S::Complete, S::Active, R::UserReopened),
        (S::Infeasible, S::Active, R::ModelReopened),
        (S::Infeasible, S::Active, R::UserReopened),
    ];
    for (from, to, reason) in cases {
        assert!(
            from.can_transition_to(*to, *reason),
            "{from:?} -> {to:?} via {reason:?} must be legal"
        );
    }
}

#[test]
fn terminal_states_only_exit_to_active_via_reopen_reasons() {
    for &terminal in TERMINAL_STATUSES {
        for &to in ALL_STATUSES {
            for &reason in NON_REOPEN_REASONS {
                assert!(
                    !terminal.can_transition_to(to, reason),
                    "{terminal:?} must not transition to {to:?} via non-reopen {reason:?}"
                );
            }
        }
        for &reason in REOPEN_REASONS {
            assert!(
                terminal.can_transition_to(ThreadGoalStatus::Active, reason),
                "{terminal:?} -> Active via {reason:?} must be legal"
            );
            for &other in &[
                ThreadGoalStatus::Paused,
                ThreadGoalStatus::Complete,
                ThreadGoalStatus::Infeasible,
            ] {
                assert!(
                    !terminal.can_transition_to(other, reason),
                    "{terminal:?} -> {other:?} via {reason:?} must be illegal"
                );
            }
        }
    }
}

#[test]
fn reopen_reasons_only_apply_from_terminal() {
    for &reason in REOPEN_REASONS {
        for &from in &[ThreadGoalStatus::Active, ThreadGoalStatus::Paused] {
            for &to in ALL_STATUSES {
                assert!(
                    !from.can_transition_to(to, reason),
                    "{from:?} -> {to:?} via {reason:?} must be illegal (reopen only valid from terminal)"
                );
            }
        }
    }
}

#[test]
fn self_transitions_are_never_legal() {
    for &state in ALL_STATUSES {
        for &reason in ALL_REASONS {
            assert!(
                !state.can_transition_to(state, reason),
                "{state:?} -> {state:?} via {reason:?} must be illegal"
            );
        }
    }
}

#[test]
fn participates_in_continuation_only_when_active() {
    assert!(ThreadGoalStatus::Active.participates_in_continuation());
    assert!(!ThreadGoalStatus::Paused.participates_in_continuation());
    assert!(!ThreadGoalStatus::Complete.participates_in_continuation());
    assert!(!ThreadGoalStatus::Infeasible.participates_in_continuation());
}

#[test]
fn is_terminal_covers_complete_and_infeasible() {
    assert!(!ThreadGoalStatus::Active.is_terminal());
    assert!(!ThreadGoalStatus::Paused.is_terminal());
    assert!(ThreadGoalStatus::Complete.is_terminal());
    assert!(ThreadGoalStatus::Infeasible.is_terminal());
}

#[test]
fn status_string_roundtrip_through_serde() {
    let pairs = [
        (ThreadGoalStatus::Active, "active"),
        (ThreadGoalStatus::Paused, "paused"),
        (ThreadGoalStatus::Complete, "complete"),
        (ThreadGoalStatus::Infeasible, "infeasible"),
    ];
    for (status, expected) in pairs {
        assert_eq!(status.as_str(), expected);
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: ThreadGoalStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}

#[test]
fn infeasible_reasons_roundtrip_through_serde() {
    let pairs = [
        (GoalTransitionReason::ModelInfeasible, "model_infeasible"),
        (GoalTransitionReason::UserInfeasible, "user_infeasible"),
    ];
    for (reason, expected) in pairs {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: GoalTransitionReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, reason);
    }
}

#[test]
fn legacy_goal_json_without_infeasible_still_deserializes() {
    let legacy = r#"{"session_id":"s","goal_id":"g","objective":"obj","status":"active","created_at":"2026-05-20T00:00:00Z","updated_at":"2026-05-20T00:00:00Z"}"#;
    let goal: ThreadGoal = serde_json::from_str(legacy).unwrap();
    assert_eq!(goal.status, ThreadGoalStatus::Active);
}

#[test]
fn new_goal_starts_active_with_unique_id() {
    let g1 = ThreadGoal::new("s", "objective one");
    let g2 = ThreadGoal::new("s", "objective two");
    assert_eq!(g1.status, ThreadGoalStatus::Active);
    assert_ne!(g1.goal_id, g2.goal_id);
}
