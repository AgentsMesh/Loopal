use loopal_protocol::{AgentEvent, AgentEventPayload};

#[test]
fn test_event_stream_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Stream {
        text: "hello".into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::Stream { text } = deserialized.payload {
        assert_eq!(text, "hello");
    } else {
        panic!("expected AgentEventPayload::Stream");
    }
}

#[test]
fn test_event_tool_call_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc_1".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/tmp/test.rs"}),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::ToolCall { id, name, input } = deserialized.payload {
        assert_eq!(id, "tc_1");
        assert_eq!(name, "Read");
        assert_eq!(input["file_path"], "/tmp/test.rs");
    } else {
        panic!("expected AgentEventPayload::ToolCall");
    }
}

#[test]
fn test_event_tool_result_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc_1".into(),
        name: "Read".into(),
        result: "file contents".into(),
        is_error: false,
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::ToolResult {
        id,
        name,
        result,
        is_error,
    } = deserialized.payload
    {
        assert_eq!(id, "tc_1");
        assert_eq!(name, "Read");
        assert_eq!(result, "file contents");
        assert!(!is_error);
    } else {
        panic!("expected AgentEventPayload::ToolResult");
    }
}

#[test]
fn test_event_tool_result_error_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc_2".into(),
        name: "Bash".into(),
        result: "command not found".into(),
        is_error: true,
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::ToolResult { is_error, .. } = deserialized.payload {
        assert!(is_error);
    } else {
        panic!("expected AgentEventPayload::ToolResult");
    }
}

#[test]
fn test_event_tool_permission_request_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
        id: "tc_3".into(),
        name: "Write".into(),
        input: serde_json::json!({"file_path": "/tmp/out.txt"}),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::ToolPermissionRequest { id, name, input } =
        deserialized.payload
    {
        assert_eq!(id, "tc_3");
        assert_eq!(name, "Write");
        assert_eq!(input["file_path"], "/tmp/out.txt");
    } else {
        panic!("expected AgentEventPayload::ToolPermissionRequest");
    }
}

#[test]
fn test_event_error_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Error {
        message: "something failed".into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::Error { message } = deserialized.payload {
        assert_eq!(message, "something failed");
    } else {
        panic!("expected AgentEventPayload::Error");
    }
}

#[test]
fn test_event_awaiting_input_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::AwaitingInput);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized.payload, AgentEventPayload::AwaitingInput));
}

#[test]
fn test_event_max_turns_reached_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::MaxTurnsReached { turns: 50 });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::MaxTurnsReached { turns } = deserialized.payload {
        assert_eq!(turns, 50);
    } else {
        panic!("expected AgentEventPayload::MaxTurnsReached");
    }
}

#[test]
fn test_event_token_usage_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 1000,
        output_tokens: 500,
        context_window: 200_000,
        cache_creation_input_tokens: 50,
        cache_read_input_tokens: 800,
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::TokenUsage {
        input_tokens,
        output_tokens,
        context_window,
        cache_creation_input_tokens,
        cache_read_input_tokens,
    } = deserialized.payload
    {
        assert_eq!(input_tokens, 1000);
        assert_eq!(output_tokens, 500);
        assert_eq!(context_window, 200_000);
        assert_eq!(cache_creation_input_tokens, 50);
        assert_eq!(cache_read_input_tokens, 800);
    } else {
        panic!("expected AgentEventPayload::TokenUsage");
    }
}

#[test]
fn test_event_mode_changed_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::ModeChanged {
        mode: "plan".into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::ModeChanged { mode } = deserialized.payload {
        assert_eq!(mode, "plan");
    } else {
        panic!("expected AgentEventPayload::ModeChanged");
    }
}

#[test]
fn test_event_started_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Started);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized.payload, AgentEventPayload::Started));
}

#[test]
fn test_event_finished_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Finished);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized.payload, AgentEventPayload::Finished));
}

#[test]
fn test_event_rewound_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Rewound { remaining_turns: 3 });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::Rewound { remaining_turns } = deserialized.payload {
        assert_eq!(remaining_turns, 3);
    } else {
        panic!("expected AgentEventPayload::Rewound");
    }
}
