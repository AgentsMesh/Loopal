use loopal_protocol::{AgentEventPayload, GoalTransitionReason, ThreadGoal};
use loopal_view_state::ViewStateReducer;

fn update(goal: &ThreadGoal) -> AgentEventPayload {
    AgentEventPayload::ThreadGoalUpdated {
        goal: Some(goal.clone()),
        reason: GoalTransitionReason::UserCreated,
    }
}

#[test]
fn thread_goal_update_is_idempotent_and_resume_clears_it() {
    let mut reducer = ViewStateReducer::new("main");
    let goal = ThreadGoal::new("session-1", "finish workflow");
    assert_eq!(reducer.apply(update(&goal)), Some(1));
    assert_eq!(reducer.state().thread_goal.as_ref(), Some(&goal));

    assert_eq!(reducer.apply(update(&goal)), None);
    assert_eq!(reducer.rev(), 1);

    reducer.apply(AgentEventPayload::SessionResumed {
        session_id: "session-2".into(),
        message_count: 0,
    });
    assert!(reducer.state().thread_goal.is_none());
}
