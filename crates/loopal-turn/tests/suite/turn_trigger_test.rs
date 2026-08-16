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
    let workflow = TurnTrigger::WorkflowResult {
        session_id: "session".into(),
        run_id: "wrun_one".into(),
        terminal_revision: 1,
        payload_digest: format!("sha256:{}", "0".repeat(64)),
        state: "succeeded".into(),
        content: "done".into(),
    };

    assert!(goal.is_goal_continuation());
    for t in [user, cron, agent, channel, hook, workflow.clone(), resume] {
        assert!(
            !t.is_goal_continuation(),
            "non-GoalContinuation trigger must not be flagged: {t:?}"
        );
    }

    let encoded = serde_json::to_value(&workflow).unwrap();
    let decoded: TurnTrigger = serde_json::from_value(encoded).unwrap();
    assert!(matches!(
        decoded,
        TurnTrigger::WorkflowResult { payload_digest, .. }
            if payload_digest.starts_with("sha256:")
    ));
}
