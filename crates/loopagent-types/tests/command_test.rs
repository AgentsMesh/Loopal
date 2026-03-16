use loopagent_types::command::{AgentMode, UserCommand};

#[test]
fn test_user_command_message_contains_string() {
    let cmd = UserCommand::Message("hello world".to_string());
    if let UserCommand::Message(s) = cmd {
        assert_eq!(s, "hello world");
    } else {
        panic!("expected UserCommand::Message");
    }
}

#[test]
fn test_user_command_message_empty_string() {
    let cmd = UserCommand::Message(String::new());
    if let UserCommand::Message(s) = cmd {
        assert!(s.is_empty());
    } else {
        panic!("expected UserCommand::Message");
    }
}

#[test]
fn test_user_command_mode_switch_plan() {
    let cmd = UserCommand::ModeSwitch(AgentMode::Plan);
    if let UserCommand::ModeSwitch(mode) = cmd {
        assert_eq!(mode, AgentMode::Plan);
    } else {
        panic!("expected UserCommand::ModeSwitch");
    }
}

#[test]
fn test_user_command_mode_switch_act() {
    let cmd = UserCommand::ModeSwitch(AgentMode::Act);
    if let UserCommand::ModeSwitch(mode) = cmd {
        assert_eq!(mode, AgentMode::Act);
    } else {
        panic!("expected UserCommand::ModeSwitch");
    }
}

#[test]
fn test_agent_mode_equality() {
    assert_eq!(AgentMode::Act, AgentMode::Act);
    assert_eq!(AgentMode::Plan, AgentMode::Plan);
    assert_ne!(AgentMode::Act, AgentMode::Plan);
    assert_ne!(AgentMode::Plan, AgentMode::Act);
}

#[test]
fn test_agent_mode_clone() {
    let mode = AgentMode::Plan;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_user_command_clone() {
    let cmd = UserCommand::Message("test".to_string());
    let cloned = cmd.clone();
    if let UserCommand::Message(s) = cloned {
        assert_eq!(s, "test");
    } else {
        panic!("expected UserCommand::Message");
    }
}

#[test]
fn test_agent_mode_debug() {
    let act = format!("{:?}", AgentMode::Act);
    let plan = format!("{:?}", AgentMode::Plan);
    assert_eq!(act, "Act");
    assert_eq!(plan, "Plan");
}
