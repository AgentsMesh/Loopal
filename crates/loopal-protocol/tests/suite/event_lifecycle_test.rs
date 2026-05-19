use loopal_protocol::{
    AgentEvent, AgentEventPayload, CompactionSummary, SubAgentSpawn, TurnSummary,
};

#[test]
fn test_event_cleared_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Cleared {
        context_window: 200_000,
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::Cleared { context_window } = de.payload else {
        panic!("expected Cleared");
    };
    assert_eq!(context_window, 200_000);
}

#[test]
fn test_event_model_changed_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::ModelChanged {
        model: "claude-opus-4-7".into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::ModelChanged { model } = de.payload else {
        panic!("expected ModelChanged");
    };
    assert_eq!(model, "claude-opus-4-7");
}

#[test]
fn test_event_thinking_changed_serde_roundtrip() {
    let raw = r#"{"type":"effort","level":"high"}"#;
    let event = AgentEvent::root(AgentEventPayload::ThinkingChanged {
        thinking_config: raw.into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::ThinkingChanged { thinking_config } = de.payload else {
        panic!("expected ThinkingChanged");
    };
    assert_eq!(thinking_config, raw);
}

// --- newtype variant wire-format compatibility ---
//
// `TurnCompleted` / `Compacted` / `SubAgentSpawned` were refactored from
// struct-form to newtype-form (`Variant(SomeStruct)`) to keep
// `event_payload.rs` under the 200-line cap. The wire format MUST stay
// identical so older clients and Hub-side reducers can still decode
// these events. These tests pin the on-disk shape.

#[test]
fn test_turn_completed_wire_format_unchanged() {
    let event = AgentEvent::root(AgentEventPayload::TurnCompleted(TurnSummary {
        turn_id: 7,
        duration_ms: 1234,
        llm_calls: 2,
        tool_calls_requested: 3,
        tool_calls_approved: 3,
        tool_calls_denied: 0,
        tool_errors: 0,
        auto_continuations: 0,
        warnings_injected: 0,
        tokens_in: 100,
        tokens_out: 50,
        modified_files: vec!["a.rs".into()],
    }));
    let json = serde_json::to_string(&event).unwrap();
    // Wire shape pinned: the inner struct fields are flattened under the
    // variant name, exactly like the prior struct-form encoding.
    assert!(
        json.contains(r#""TurnCompleted":{"turn_id":7"#),
        "wire shape regressed: {json}"
    );
    assert!(json.contains(r#""modified_files":["a.rs"]"#));
}

#[test]
fn test_compacted_wire_format_unchanged() {
    let event = AgentEvent::root(AgentEventPayload::Compacted(CompactionSummary {
        kept: 5,
        removed: 10,
        tokens_before: 1000,
        tokens_after: 500,
        strategy: "smart".into(),
        summary_msg_id: None,
        files_rehydrated: 0,
    }));
    let json = serde_json::to_string(&event).unwrap();
    assert!(
        json.contains(r#""Compacted":{"kept":5,"removed":10"#),
        "wire shape regressed: {json}"
    );
}

#[test]
fn test_sub_agent_spawned_wire_format_unchanged() {
    let event = AgentEvent::root(AgentEventPayload::SubAgentSpawned(SubAgentSpawn {
        name: "researcher".into(),
        agent_id: "a-1".into(),
        parent: None,
        model: Some("claude-sonnet".into()),
        session_id: None,
    }));
    let json = serde_json::to_string(&event).unwrap();
    assert!(
        json.contains(r#""SubAgentSpawned":{"name":"researcher""#),
        "wire shape regressed: {json}"
    );
    // `parent` and `session_id` are None → omitted via skip_serializing_if.
    assert!(!json.contains(r#""parent""#));
    assert!(!json.contains(r#""session_id""#));
}

#[test]
fn test_sub_agent_spawned_legacy_struct_form_still_deserializes() {
    // The wire form a v1 sender would have emitted (struct variant form).
    // Newtype-wrapping `SubAgentSpawn` must keep this shape decodable so
    // mixed-version clusters during rollout don't drop events.
    let legacy = r#"{
        "agent_name": null,
        "payload": {
            "SubAgentSpawned": {
                "name": "researcher",
                "agent_id": "a-1",
                "model": "claude-sonnet"
            }
        }
    }"#;
    let event: AgentEvent = serde_json::from_str(legacy).unwrap();
    let AgentEventPayload::SubAgentSpawned(s) = event.payload else {
        panic!("expected SubAgentSpawned");
    };
    assert_eq!(s.name, "researcher");
    assert_eq!(s.model.as_deref(), Some("claude-sonnet"));
    assert!(s.parent.is_none());
    assert!(s.session_id.is_none());
}
