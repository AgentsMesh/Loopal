use loopagent_types::command::AgentMode;
use loopagent_types::control::ControlCommand;

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
    let cmd = ControlCommand::Compact;
    assert!(matches!(cmd, ControlCommand::Compact));
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
