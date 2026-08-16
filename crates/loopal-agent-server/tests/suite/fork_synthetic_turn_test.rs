use loopal_agent_server::testing::{
    StartParams, build_fork_synthetic_turn, initial_turns_for_start,
};

fn blank_start() -> StartParams {
    StartParams {
        lifecycle: loopal_runtime::LifecycleMode::Ephemeral,
        ..StartParams::default()
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
    assert!(matches!(turn.outcome, loopal_turn::TurnOutcome::InProgress));
    assert_eq!(turn.body.steps.len(), 1);
    match &turn.body.steps[0] {
        loopal_turn::TurnStep::Injection { kind, text } => {
            assert!(matches!(kind, loopal_turn::InjectionKind::SystemNote));
            assert!(text.contains("fork msg 1"));
            assert!(text.contains("assigned child task") && text.contains("forked worker"));
        }
        _ => panic!("expected Injection step"),
    }
}

#[test]
fn fork_kickoff_is_persisted_for_crash_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let manager = loopal_runtime::SessionManager::with_base_dir(temp.path().to_path_buf());
    let session = manager
        .create_session_with_id(temp.path(), "test-model", "fork-session")
        .unwrap();
    let mut start = blank_start();
    start.fork_context = Some(
        serde_json::to_value(vec![loopal_provider_api::Message::user("parent context")]).unwrap(),
    );
    start.prompt = Some("assigned task".into());

    let initial = initial_turns_for_start(&start, &manager, &session.id, Vec::new()).unwrap();
    assert_eq!(initial.len(), 1);
    assert!(matches!(
        initial[0].outcome,
        loopal_turn::TurnOutcome::InProgress
    ));

    let recovered = manager.load_turns(&session.id).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].body.steps.len(), 1);
    assert!(matches!(
        recovered[0].outcome,
        loopal_turn::TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::CrashRecovery
        }
    ));
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
fn none_on_bad_or_empty_context() {
    let mut start = blank_start();
    start.fork_context = Some(serde_json::json!({"not_a_message_array": true}));
    assert!(build_fork_synthetic_turn(&start).is_none());

    start.fork_context = Some(serde_json::json!([]));
    assert!(build_fork_synthetic_turn(&start).is_none());
}

#[test]
fn renders_roles_tool_calls_results_and_ignores_non_text_blocks() {
    use loopal_provider_api::{ContentBlock, Message, MessageRole};

    let mut start = blank_start();
    start.fork_context = Some(
        serde_json::to_value(vec![
            Message::assistant("assistant text"),
            Message::system("system text"),
            Message {
                id: None,
                role: MessageRole::User,
                content: vec![
                    ContentBlock::ToolUse {
                        id: "call".into(),
                        name: "Read".into(),
                        input: serde_json::json!({"file_path": "a.rs"}),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id: "call".into(),
                        content: "ok".into(),
                        images: Vec::new(),
                        is_error: false,
                        metadata: None,
                    },
                    ContentBlock::ToolResult {
                        tool_use_id: "bad".into(),
                        content: "failed".into(),
                        images: Vec::new(),
                        is_error: true,
                        metadata: None,
                    },
                    ContentBlock::Thinking {
                        thinking: "hidden".into(),
                        signature: None,
                    },
                ],
                origin: None,
                ephemeral_in_history: false,
            },
        ])
        .unwrap(),
    );

    let turn = build_fork_synthetic_turn(&start).unwrap();
    let loopal_turn::TurnStep::Injection { text, .. } = &turn.body.steps[0] else {
        panic!("expected injection")
    };
    assert!(text.contains("ASSISTANT: assistant text"));
    assert!(text.contains("SYSTEM: system text"));
    assert!(text.contains("[calls Read({\"file_path\":\"a.rs\"})]"));
    assert!(text.contains("[tool result: ok]"));
    assert!(text.contains("[tool error: failed]"));
    assert!(!text.contains("hidden"));
}
