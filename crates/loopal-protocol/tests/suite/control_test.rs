use loopal_protocol::{AgentMode, ControlCommand};
use strum::IntoEnumIterator;

#[test]
fn test_control_command_mode_switch() {
    let cmd = ControlCommand::ModeSwitch(AgentMode::Plan);
    assert!(matches!(cmd, ControlCommand::ModeSwitch(AgentMode::Plan)));
}

#[test]
fn test_control_command_clear() {
    let cmd = ControlCommand::Clear;
    assert!(matches!(cmd, ControlCommand::Clear));
}

#[test]
fn test_control_command_compact() {
    let cmd = ControlCommand::Compact { instructions: None };
    assert!(matches!(cmd, ControlCommand::Compact { .. }));
}

#[test]
fn test_control_command_compact_carries_instructions() {
    let cmd = ControlCommand::Compact {
        instructions: Some("preserve repro steps".into()),
    };
    if let ControlCommand::Compact { instructions } = cmd {
        assert_eq!(instructions.as_deref(), Some("preserve repro steps"));
    } else {
        panic!("expected Compact variant");
    }
}

#[test]
fn test_control_command_model_switch() {
    let cmd = ControlCommand::ModelSwitch("gpt-4".to_string());
    if let ControlCommand::ModelSwitch(model) = cmd {
        assert_eq!(model, "gpt-4");
    } else {
        panic!("expected ModelSwitch");
    }
}

#[test]
fn test_control_command_clone() {
    let cmd = ControlCommand::ModelSwitch("test".to_string());
    let cloned = cmd.clone();
    assert!(matches!(cloned, ControlCommand::ModelSwitch(_)));
}

#[test]
fn test_control_command_rewind() {
    let cmd = ControlCommand::Rewind { turn_index: 3 };
    if let ControlCommand::Rewind { turn_index } = cmd {
        assert_eq!(turn_index, 3);
    } else {
        panic!("expected Rewind");
    }
}

#[test]
fn test_control_command_thinking_switch() {
    let json = r#"{"type":"effort","level":"high"}"#.to_string();
    let cmd = ControlCommand::ThinkingSwitch(json.clone());
    if let ControlCommand::ThinkingSwitch(val) = cmd {
        assert_eq!(val, json);
    } else {
        panic!("expected ThinkingSwitch");
    }
}

#[test]
fn test_control_command_resume_session() {
    let cmd = ControlCommand::ResumeSession("abc-123".to_string());
    if let ControlCommand::ResumeSession(sid) = cmd {
        assert_eq!(sid, "abc-123");
    } else {
        panic!("expected ResumeSession");
    }
}

#[test]
fn test_control_command_resume_session_serde_roundtrip() {
    let cmd = ControlCommand::ResumeSession("session-xyz".to_string());
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ControlCommand = serde_json::from_str(&json).unwrap();
    if let ControlCommand::ResumeSession(sid) = deserialized {
        assert_eq!(sid, "session-xyz");
    } else {
        panic!("expected ResumeSession after roundtrip");
    }
}

#[test]
fn test_control_command_mcp_disconnect_serde_roundtrip() {
    let cmd = ControlCommand::McpDisconnect {
        server: "my-mcp".to_string(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ControlCommand = serde_json::from_str(&json).unwrap();
    if let ControlCommand::McpDisconnect { server } = deserialized {
        assert_eq!(server, "my-mcp");
    } else {
        panic!("expected McpDisconnect after roundtrip");
    }
}

#[test]
fn test_control_command_bg_task_kill_serde_roundtrip() {
    let cmd = ControlCommand::BgTaskKill {
        id: "bg_42".to_string(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ControlCommand = serde_json::from_str(&json).unwrap();
    if let ControlCommand::BgTaskKill { id } = deserialized {
        assert_eq!(id, "bg_42");
    } else {
        panic!("expected BgTaskKill after roundtrip");
    }
}

#[test]
fn test_control_command_cron_delete_serde_roundtrip() {
    let cmd = ControlCommand::CronDelete {
        id: "abc12345".to_string(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ControlCommand = serde_json::from_str(&json).unwrap();
    if let ControlCommand::CronDelete { id } = deserialized {
        assert_eq!(id, "abc12345");
    } else {
        panic!("expected CronDelete after roundtrip");
    }
}

#[test]
fn suspend_and_resume_roundtrip_through_json() {
    for cmd in [ControlCommand::Suspend, ControlCommand::Unsuspend] {
        let json = serde_json::to_string(&cmd).unwrap();
        let restored: ControlCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(
            std::mem::discriminant(&restored),
            std::mem::discriminant(&cmd)
        );
    }
}

#[test]
fn test_all_control_commands_serde_roundtrip() {
    // Reflective: every variant must survive JSON roundtrip. Discriminant
    // equality is enough — payload fidelity for specific variants is
    // covered by the dedicated tests above. The point of iterating here is
    // to catch a future variant that forgets a serde-friendly type or that
    // breaks our public IPC contract.
    for original in ControlCommand::iter() {
        let json = serde_json::to_string(&original)
            .unwrap_or_else(|_| panic!("variant {original:?} failed to serialize"));
        let restored: ControlCommand = serde_json::from_str(&json)
            .unwrap_or_else(|_| panic!("variant {original:?} failed to deserialize from {json}"));
        assert_eq!(
            std::mem::discriminant(&restored),
            std::mem::discriminant(&original),
            "discriminant changed across roundtrip for {original:?}"
        );
    }
}
