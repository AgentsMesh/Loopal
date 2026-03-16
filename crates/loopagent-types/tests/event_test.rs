use loopagent_types::event::AgentEvent;

#[test]
fn test_event_stream_serde_roundtrip() {
    let event = AgentEvent::Stream {
        text: "hello".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::Stream { text } = deserialized {
        assert_eq!(text, "hello");
    } else {
        panic!("expected AgentEvent::Stream");
    }
}

#[test]
fn test_event_tool_call_serde_roundtrip() {
    let event = AgentEvent::ToolCall {
        id: "tc_1".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/tmp/test.rs"}),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::ToolCall { id, name, input } = deserialized {
        assert_eq!(id, "tc_1");
        assert_eq!(name, "Read");
        assert_eq!(input["file_path"], "/tmp/test.rs");
    } else {
        panic!("expected AgentEvent::ToolCall");
    }
}

#[test]
fn test_event_tool_result_serde_roundtrip() {
    let event = AgentEvent::ToolResult {
        id: "tc_1".into(),
        name: "Read".into(),
        result: "file contents".into(),
        is_error: false,
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::ToolResult {
        id,
        name,
        result,
        is_error,
    } = deserialized
    {
        assert_eq!(id, "tc_1");
        assert_eq!(name, "Read");
        assert_eq!(result, "file contents");
        assert!(!is_error);
    } else {
        panic!("expected AgentEvent::ToolResult");
    }
}

#[test]
fn test_event_tool_result_error_serde_roundtrip() {
    let event = AgentEvent::ToolResult {
        id: "tc_2".into(),
        name: "Bash".into(),
        result: "command not found".into(),
        is_error: true,
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::ToolResult { is_error, .. } = deserialized {
        assert!(is_error);
    } else {
        panic!("expected AgentEvent::ToolResult");
    }
}

#[test]
fn test_event_tool_permission_request_serde_roundtrip() {
    let event = AgentEvent::ToolPermissionRequest {
        id: "tc_3".into(),
        name: "Write".into(),
        input: serde_json::json!({"file_path": "/tmp/out.txt"}),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::ToolPermissionRequest { id, name, input } = deserialized {
        assert_eq!(id, "tc_3");
        assert_eq!(name, "Write");
        assert_eq!(input["file_path"], "/tmp/out.txt");
    } else {
        panic!("expected AgentEvent::ToolPermissionRequest");
    }
}

#[test]
fn test_event_error_serde_roundtrip() {
    let event = AgentEvent::Error {
        message: "something failed".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::Error { message } = deserialized {
        assert_eq!(message, "something failed");
    } else {
        panic!("expected AgentEvent::Error");
    }
}

#[test]
fn test_event_awaiting_input_serde_roundtrip() {
    let event = AgentEvent::AwaitingInput;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, AgentEvent::AwaitingInput));
}

#[test]
fn test_event_max_turns_reached_serde_roundtrip() {
    let event = AgentEvent::MaxTurnsReached { turns: 50 };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::MaxTurnsReached { turns } = deserialized {
        assert_eq!(turns, 50);
    } else {
        panic!("expected AgentEvent::MaxTurnsReached");
    }
}

#[test]
fn test_event_token_usage_serde_roundtrip() {
    let event = AgentEvent::TokenUsage {
        input_tokens: 1000,
        output_tokens: 500,
        context_window: 200_000,
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::TokenUsage {
        input_tokens,
        output_tokens,
        context_window,
    } = deserialized
    {
        assert_eq!(input_tokens, 1000);
        assert_eq!(output_tokens, 500);
        assert_eq!(context_window, 200_000);
    } else {
        panic!("expected AgentEvent::TokenUsage");
    }
}

#[test]
fn test_event_mode_changed_serde_roundtrip() {
    let event = AgentEvent::ModeChanged {
        mode: "plan".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEvent::ModeChanged { mode } = deserialized {
        assert_eq!(mode, "plan");
    } else {
        panic!("expected AgentEvent::ModeChanged");
    }
}

#[test]
fn test_event_started_serde_roundtrip() {
    let event = AgentEvent::Started;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, AgentEvent::Started));
}

#[test]
fn test_event_finished_serde_roundtrip() {
    let event = AgentEvent::Finished;
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, AgentEvent::Finished));
}
