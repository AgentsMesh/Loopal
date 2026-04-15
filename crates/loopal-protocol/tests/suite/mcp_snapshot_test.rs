use loopal_protocol::{AgentEventPayload, ControlCommand, McpServerSnapshot};

fn sample_snapshot() -> McpServerSnapshot {
    McpServerSnapshot {
        name: "test-server".into(),
        transport: "stdio".into(),
        source: "project".into(),
        status: "connected".into(),
        tool_count: 5,
        resource_count: 2,
        prompt_count: 1,
        errors: vec![],
    }
}

#[test]
fn test_mcp_server_snapshot_serde_roundtrip() {
    let snap = sample_snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let deserialized: McpServerSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "test-server");
    assert_eq!(deserialized.tool_count, 5);
    assert_eq!(deserialized.source, "project");
}

#[test]
fn test_mcp_server_snapshot_with_errors() {
    let snap = McpServerSnapshot {
        errors: vec!["timeout".into(), "auth failed".into()],
        ..sample_snapshot()
    };
    let json = serde_json::to_string(&snap).unwrap();
    let deserialized: McpServerSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.errors.len(), 2);
}

#[test]
fn test_control_command_query_mcp_status() {
    let cmd = ControlCommand::QueryMcpStatus;
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ControlCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, ControlCommand::QueryMcpStatus));
}

#[test]
fn test_control_command_mcp_reconnect() {
    let cmd = ControlCommand::McpReconnect {
        server: "my-server".into(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ControlCommand = serde_json::from_str(&json).unwrap();
    if let ControlCommand::McpReconnect { server } = deserialized {
        assert_eq!(server, "my-server");
    } else {
        panic!("expected McpReconnect");
    }
}

#[test]
fn test_mcp_status_report_event_serde() {
    let event = AgentEventPayload::McpStatusReport {
        servers: vec![sample_snapshot()],
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEventPayload = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::McpStatusReport { servers } = deserialized {
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "test-server");
    } else {
        panic!("expected McpStatusReport");
    }
}
