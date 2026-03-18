use loopagent_types::agent_input::AgentInput;
use loopagent_types::control::ControlCommand;
use loopagent_types::command::AgentMode;
use loopagent_types::envelope::{Envelope, MessageSource};

#[test]
fn test_agent_input_message_from_human() {
    let env = Envelope::new(MessageSource::Human, "main", "hello");
    let input = AgentInput::Message(env);
    assert!(matches!(input, AgentInput::Message(_)));
}

#[test]
fn test_agent_input_message_from_agent() {
    let env = Envelope::new(MessageSource::Agent("researcher".into()), "main", "found it");
    let input = AgentInput::Message(env);
    if let AgentInput::Message(e) = input {
        assert_eq!(e.content, "found it");
        assert!(matches!(e.source, MessageSource::Agent(ref n) if n == "researcher"));
    } else {
        panic!("expected AgentInput::Message");
    }
}

#[test]
fn test_agent_input_control_mode_switch() {
    let input = AgentInput::Control(ControlCommand::ModeSwitch(AgentMode::Plan));
    assert!(matches!(
        input,
        AgentInput::Control(ControlCommand::ModeSwitch(AgentMode::Plan))
    ));
}

#[test]
fn test_agent_input_control_clear() {
    let input = AgentInput::Control(ControlCommand::Clear);
    assert!(matches!(input, AgentInput::Control(ControlCommand::Clear)));
}

#[test]
fn test_agent_input_control_compact() {
    let input = AgentInput::Control(ControlCommand::Compact);
    assert!(matches!(input, AgentInput::Control(ControlCommand::Compact)));
}

#[test]
fn test_agent_input_control_model_switch() {
    let input = AgentInput::Control(ControlCommand::ModelSwitch("gpt-4".into()));
    if let AgentInput::Control(ControlCommand::ModelSwitch(m)) = input {
        assert_eq!(m, "gpt-4");
    } else {
        panic!("expected AgentInput::Control(ModelSwitch)");
    }
}
