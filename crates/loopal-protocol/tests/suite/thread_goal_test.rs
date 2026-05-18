use loopal_protocol::thread_goal::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};

const ALL_STATUSES: &[ThreadGoalStatus] = &[
    ThreadGoalStatus::Active,
    ThreadGoalStatus::Paused,
    ThreadGoalStatus::Complete,
];

const NON_REOPEN_REASONS: &[GoalTransitionReason] = &[
    GoalTransitionReason::UserCreated,
    GoalTransitionReason::ModelCompleted,
    GoalTransitionReason::UserCompleted,
    GoalTransitionReason::UserPaused,
    GoalTransitionReason::UserResumed,
    GoalTransitionReason::UserCleared,
    GoalTransitionReason::BarrenContinuation,
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
    GoalTransitionReason::BarrenContinuation,
];

#[test]
fn each_legal_transition_is_accepted() {
    let cases: &[(ThreadGoalStatus, ThreadGoalStatus, GoalTransitionReason)] = &[
        (
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        ),
        (
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Complete,
            GoalTransitionReason::UserCompleted,
        ),
        (
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Complete,
            GoalTransitionReason::BarrenContinuation,
        ),
        (
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Paused,
            GoalTransitionReason::UserPaused,
        ),
        (
            ThreadGoalStatus::Paused,
            ThreadGoalStatus::Active,
            GoalTransitionReason::UserResumed,
        ),
        (
            ThreadGoalStatus::Paused,
            ThreadGoalStatus::Complete,
            GoalTransitionReason::UserCompleted,
        ),
        (
            ThreadGoalStatus::Complete,
            ThreadGoalStatus::Active,
            GoalTransitionReason::ModelReopened,
        ),
        (
            ThreadGoalStatus::Complete,
            ThreadGoalStatus::Active,
            GoalTransitionReason::UserReopened,
        ),
    ];
    for (from, to, reason) in cases {
        assert!(
            from.can_transition_to(*to, *reason),
            "{from:?} -> {to:?} via {reason:?} must be legal"
        );
    }
}

#[test]
fn complete_only_exits_to_active_via_reopen_reasons() {
    for &to in ALL_STATUSES {
        for &reason in NON_REOPEN_REASONS {
            assert!(
                !ThreadGoalStatus::Complete.can_transition_to(to, reason),
                "Complete must not transition to {to:?} via non-reopen {reason:?}"
            );
        }
    }
    for &reason in REOPEN_REASONS {
        assert!(
            ThreadGoalStatus::Complete.can_transition_to(ThreadGoalStatus::Active, reason),
            "Complete -> Active via {reason:?} must be legal"
        );
        assert!(
            !ThreadGoalStatus::Complete.can_transition_to(ThreadGoalStatus::Paused, reason),
            "Complete -> Paused via {reason:?} must be illegal"
        );
    }
}

#[test]
fn reopen_reasons_only_apply_from_complete() {
    for &reason in REOPEN_REASONS {
        for &from in &[ThreadGoalStatus::Active, ThreadGoalStatus::Paused] {
            for &to in ALL_STATUSES {
                assert!(
                    !from.can_transition_to(to, reason),
                    "{from:?} -> {to:?} via {reason:?} must be illegal (reopen only valid from Complete)"
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
}

#[test]
fn status_string_roundtrip_through_serde() {
    let pairs = [
        (ThreadGoalStatus::Active, "active"),
        (ThreadGoalStatus::Paused, "paused"),
        (ThreadGoalStatus::Complete, "complete"),
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
fn reopen_reasons_roundtrip_through_serde() {
    let pairs = [
        (GoalTransitionReason::ModelReopened, "model_reopened"),
        (GoalTransitionReason::UserReopened, "user_reopened"),
    ];
    for (reason, expected) in pairs {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: GoalTransitionReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, reason);
    }
}

#[test]
fn new_goal_starts_active_with_unique_id() {
    let g1 = ThreadGoal::new("s", "objective one");
    let g2 = ThreadGoal::new("s", "objective two");
    assert_eq!(g1.status, ThreadGoalStatus::Active);
    assert_ne!(g1.goal_id, g2.goal_id);
}
