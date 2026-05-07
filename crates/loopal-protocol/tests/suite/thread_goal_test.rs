use loopal_protocol::thread_goal::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};

const ALL_STATUSES: &[ThreadGoalStatus] = &[
    ThreadGoalStatus::Active,
    ThreadGoalStatus::Paused,
    ThreadGoalStatus::BudgetLimited,
    ThreadGoalStatus::Complete,
];

const ALL_REASONS: &[GoalTransitionReason] = &[
    GoalTransitionReason::UserCreated,
    GoalTransitionReason::ModelCompleted,
    GoalTransitionReason::UserCompleted,
    GoalTransitionReason::BudgetExhausted,
    GoalTransitionReason::UserPaused,
    GoalTransitionReason::UserResumed,
    GoalTransitionReason::UserExtendedBudget,
    GoalTransitionReason::UserCleared,
    GoalTransitionReason::BarrenContinuation,
    GoalTransitionReason::UsageUpdated,
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
            ThreadGoalStatus::BudgetLimited,
            GoalTransitionReason::BudgetExhausted,
        ),
        (
            ThreadGoalStatus::Active,
            ThreadGoalStatus::BudgetLimited,
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
            ThreadGoalStatus::BudgetLimited,
            ThreadGoalStatus::Active,
            GoalTransitionReason::UserExtendedBudget,
        ),
        (
            ThreadGoalStatus::BudgetLimited,
            ThreadGoalStatus::Complete,
            GoalTransitionReason::UserCompleted,
        ),
        (
            ThreadGoalStatus::BudgetLimited,
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
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
fn complete_is_terminal_for_every_reason() {
    for &to in ALL_STATUSES {
        for &reason in ALL_REASONS {
            assert!(
                !ThreadGoalStatus::Complete.can_transition_to(to, reason),
                "Complete must not transition to {to:?} via {reason:?}"
            );
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
fn budget_exhausted_handles_no_budget() {
    let goal = ThreadGoal::new("s", "x");
    assert!(!goal.budget_exhausted());
    assert_eq!(goal.remaining_tokens(), None);
}

#[test]
fn budget_exhausted_when_tokens_meet_budget() {
    let mut goal = ThreadGoal::new("s", "x").with_token_budget(100);
    goal.tokens_used = 100;
    assert!(goal.budget_exhausted());
    assert_eq!(goal.remaining_tokens(), Some(0));
}

#[test]
fn budget_exhausted_when_tokens_exceed_budget() {
    let mut goal = ThreadGoal::new("s", "x").with_token_budget(100);
    goal.tokens_used = 250;
    assert!(goal.budget_exhausted());
    assert_eq!(goal.remaining_tokens(), Some(0));
}

#[test]
fn participates_in_continuation_only_when_active() {
    assert!(ThreadGoalStatus::Active.participates_in_continuation());
    assert!(!ThreadGoalStatus::Paused.participates_in_continuation());
    assert!(!ThreadGoalStatus::BudgetLimited.participates_in_continuation());
    assert!(!ThreadGoalStatus::Complete.participates_in_continuation());
}

#[test]
fn status_string_roundtrip_through_serde() {
    let pairs = [
        (ThreadGoalStatus::Active, "active"),
        (ThreadGoalStatus::Paused, "paused"),
        (ThreadGoalStatus::BudgetLimited, "budget_limited"),
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
fn new_goal_starts_active_with_unique_id() {
    let g1 = ThreadGoal::new("s", "objective one");
    let g2 = ThreadGoal::new("s", "objective two");
    assert_eq!(g1.status, ThreadGoalStatus::Active);
    assert_eq!(g1.tokens_used, 0);
    assert_eq!(g1.time_used_ms, 0);
    assert_ne!(g1.goal_id, g2.goal_id);
}
