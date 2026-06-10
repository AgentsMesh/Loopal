//! Pure `subtype` → `ControlCommand` mapping for `session/control_request`.
//! Kept separate from the IO-bound handler so the full dispatch table is
//! unit-testable and `handle_control_request` stays thin. Every Loopal
//! ControlCommand the runtime implements (`input_control.rs`) is reachable here.

use loopal_protocol::{AgentMode, ControlCommand};
use serde_json::Value;

pub(crate) fn parse_loopal_control(subtype: &str, p: &Value) -> Option<ControlCommand> {
    use ControlCommand as C;
    match subtype {
        "loopal.bgTaskKill" => p["id"]
            .as_str()
            .map(|s| C::BgTaskKill { id: s.to_string() }),
        "loopal.cronDelete" => p["id"]
            .as_str()
            .map(|s| C::CronDelete { id: s.to_string() }),
        "loopal.compact" => Some(C::Compact {
            instructions: p["instructions"].as_str().map(String::from),
        }),
        "loopal.clear" => Some(C::Clear),
        "loopal.suspend" => Some(C::Suspend),
        "loopal.unsuspend" => Some(C::Unsuspend),
        "loopal.mode" => match p["mode"].as_str()? {
            "plan" => Some(C::ModeSwitch(AgentMode::Plan)),
            "act" => Some(C::ModeSwitch(AgentMode::Act)),
            _ => None,
        },
        "loopal.mcpStatus" => Some(C::QueryMcpStatus),
        "loopal.mcpReconnect" => p["server"].as_str().map(|s| C::McpReconnect {
            server: s.to_string(),
        }),
        "loopal.mcpDisconnect" => p["server"].as_str().map(|s| C::McpDisconnect {
            server: s.to_string(),
        }),
        "loopal.rewind" => p["turn_index"].as_u64().map(|n| C::Rewind {
            turn_index: n as usize,
        }),
        "loopal.resumeSession" => p["session_id"]
            .as_str()
            .map(|s| C::ResumeSession(s.to_string())),
        "loopal.thinking" => p["config"]
            .as_str()
            .map(|s| C::ThinkingSwitch(s.to_string())),
        "loopal.goalCreate" => p["objective"].as_str().map(|s| C::GoalCreate {
            objective: s.to_string(),
        }),
        "loopal.goalPause" => Some(C::GoalUserPause),
        "loopal.goalResume" => Some(C::GoalUserResume),
        "loopal.goalComplete" => Some(C::GoalUserComplete),
        "loopal.goalReopen" => Some(C::GoalUserReopen),
        "loopal.goalClear" => Some(C::GoalClear),
        "set_model" => p["model"].as_str().map(|s| C::ModelSwitch(s.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_object_and_session_controls() {
        assert!(matches!(
            parse_loopal_control("loopal.bgTaskKill", &json!({"id":"bg1"})),
            Some(ControlCommand::BgTaskKill { id }) if id == "bg1"
        ));
        assert!(matches!(
            parse_loopal_control("loopal.compact", &json!({})),
            Some(ControlCommand::Compact { instructions: None })
        ));
        assert!(matches!(
            parse_loopal_control("loopal.clear", &json!({})),
            Some(ControlCommand::Clear)
        ));
        assert!(matches!(
            parse_loopal_control("loopal.mode", &json!({"mode":"plan"})),
            Some(ControlCommand::ModeSwitch(AgentMode::Plan))
        ));
    }

    #[test]
    fn maps_mcp_suspend_goal() {
        assert!(matches!(
            parse_loopal_control("loopal.mcpReconnect", &json!({"server":"fs"})),
            Some(ControlCommand::McpReconnect { server }) if server == "fs"
        ));
        assert!(matches!(
            parse_loopal_control("loopal.suspend", &json!({})),
            Some(ControlCommand::Suspend)
        ));
        assert!(matches!(
            parse_loopal_control("loopal.goalCreate", &json!({"objective":"ship"})),
            Some(ControlCommand::GoalCreate { objective }) if objective == "ship"
        ));
        assert!(matches!(
            parse_loopal_control("loopal.goalClear", &json!({})),
            Some(ControlCommand::GoalClear)
        ));
    }

    #[test]
    fn rejects_unknown_and_missing_fields() {
        assert!(parse_loopal_control("loopal.bogus", &json!({})).is_none());
        assert!(parse_loopal_control("loopal.bgTaskKill", &json!({})).is_none());
        assert!(parse_loopal_control("loopal.mode", &json!({"mode":"bogus"})).is_none());
    }
}
