use loopal_turn::TurnTrigger;

fn env(id: &str) -> String {
    id.to_string()
}

#[test]
fn only_goal_continuation_is_goal_continuation() {
    let user = TurnTrigger::UserInput {
        envelope_id: env("e"),
        content: "hi".into(),
        images: vec![],
    };
    let cron = TurnTrigger::Cron {
        envelope_id: env("e"),
        content: "tick".into(),
    };
    let agent = TurnTrigger::Agent {
        envelope_id: env("e"),
        from: "a".into(),
        content: "c".into(),
    };
    let channel = TurnTrigger::Channel {
        envelope_id: env("e"),
        channel: "ch".into(),
        from: "a".into(),
        content: "c".into(),
    };
    let goal = TurnTrigger::GoalContinuation {
        envelope_id: env("e"),
        content: "keep going".into(),
    };
    let hook = TurnTrigger::BackgroundHook {
        envelope_id: env("e"),
        hook_kind: "stop_feedback".into(),
        content: "c".into(),
    };
    let resume = TurnTrigger::Resume;

    assert!(goal.is_goal_continuation());
    for t in [user, cron, agent, channel, hook, resume] {
        assert!(
            !t.is_goal_continuation(),
            "non-GoalContinuation trigger must not be flagged: {t:?}"
        );
    }
}
