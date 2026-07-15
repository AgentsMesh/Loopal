use loopal_agent_server::testing::{StartParams, build_fork_synthetic_turn};

fn blank_start() -> StartParams {
    StartParams {
        cwd: None,
        model: None,
        mode: None,
        prompt: None,
        permission_mode: None,
        decision_mode: None,
        no_sandbox: false,
        resume: None,
        lifecycle: loopal_runtime::LifecycleMode::Ephemeral,
        agent_type: None,
        depth: None,
        fork_context: None,
    }
}

#[test]
fn none_when_no_fork() {
    assert!(build_fork_synthetic_turn(&blank_start()).is_none());
}

#[test]
fn includes_fork_messages_and_assigned_prompt() {
    let mut start = blank_start();
    let messages = vec![loopal_provider_api::Message::user("fork msg 1")];
    start.fork_context = Some(serde_json::to_value(&messages).unwrap());
    start.prompt = Some("assigned child task".into());
    let turn = build_fork_synthetic_turn(&start).expect("should produce turn");
    assert_eq!(turn.body.steps.len(), 2);
    match &turn.body.steps[0] {
        loopal_turn::TurnStep::Injection { kind, text } => {
            assert!(matches!(kind, loopal_turn::InjectionKind::SystemNote));
            assert!(text.contains("fork msg 1"));
        }
        _ => panic!("expected Injection step"),
    }
    let loopal_turn::TurnStep::Injection { text, .. } = &turn.body.steps[1] else {
        panic!("expected assigned prompt injection")
    };
    assert!(text.contains("assigned child task") && text.contains("forked worker"));
}

#[test]
fn ignored_when_resuming() {
    let mut start = blank_start();
    start.resume = Some("sid".into());
    let messages = vec![loopal_provider_api::Message::user("fork msg")];
    start.fork_context = Some(serde_json::to_value(&messages).unwrap());
    assert!(build_fork_synthetic_turn(&start).is_none());
}

#[test]
fn none_on_bad_json() {
    let mut start = blank_start();
    start.fork_context = Some(serde_json::json!({"not_a_message_array": true}));
    assert!(build_fork_synthetic_turn(&start).is_none());
}
