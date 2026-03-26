use loopal_ipc::protocol::methods;

#[test]
fn method_names_follow_convention() {
    // Lifecycle methods
    assert_eq!(methods::INITIALIZE.name, "initialize");
    assert_eq!(methods::AGENT_START.name, "agent/start");
    assert_eq!(methods::AGENT_STATUS.name, "agent/status");
    assert_eq!(methods::AGENT_SHUTDOWN.name, "agent/shutdown");

    // Data plane
    assert_eq!(methods::AGENT_MESSAGE.name, "agent/message");

    // Control plane
    assert_eq!(methods::AGENT_CONTROL.name, "agent/control");
    assert_eq!(methods::AGENT_INTERRUPT.name, "agent/interrupt");

    // Observation plane
    assert_eq!(methods::AGENT_EVENT.name, "agent/event");

    // Bidirectional
    assert_eq!(methods::AGENT_PERMISSION.name, "agent/permission");
    assert_eq!(methods::AGENT_QUESTION.name, "agent/question");
}

#[test]
fn all_agent_methods_share_prefix() {
    let agent_methods = [
        methods::AGENT_START.name,
        methods::AGENT_STATUS.name,
        methods::AGENT_SHUTDOWN.name,
        methods::AGENT_MESSAGE.name,
        methods::AGENT_CONTROL.name,
        methods::AGENT_INTERRUPT.name,
        methods::AGENT_EVENT.name,
        methods::AGENT_PERMISSION.name,
        methods::AGENT_QUESTION.name,
    ];
    for m in agent_methods {
        assert!(
            m.starts_with("agent/"),
            "{m} should start with 'agent/'"
        );
    }
}

/// Verify protocol types from loopal-protocol can be serialized (IPC readiness).
#[test]
fn protocol_types_serialize() {
    use loopal_protocol::{AgentMode, ControlCommand, UserQuestionResponse};

    let cmd = ControlCommand::ModeSwitch(AgentMode::Plan);
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("Plan"));

    let cmd2: ControlCommand = serde_json::from_str(&json).unwrap();
    match cmd2 {
        ControlCommand::ModeSwitch(AgentMode::Plan) => {}
        _ => panic!("roundtrip failed"),
    }

    let resp = UserQuestionResponse {
        answers: vec!["yes".into()],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let resp2: UserQuestionResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(resp2.answers, vec!["yes"]);
}
