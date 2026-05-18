use async_trait::async_trait;
use loopal_protocol::ControlCommand;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

const USAGE: &str = "/goal usage: <objective> | pause | resume | complete | reopen | clear";

pub struct GoalCmd;

#[async_trait]
impl CommandHandler for GoalCmd {
    fn name(&self) -> &str {
        "/goal"
    }

    fn description(&self) -> &str {
        "Manage thread goal: /goal <objective> | pause | resume | complete | reopen | clear"
    }

    fn has_arg(&self) -> bool {
        true
    }

    async fn execute(&self, app: &mut App, arg: Option<&str>) -> CommandEffect {
        let arg = arg.unwrap_or("").trim();
        let cmd = match parse_goal_arg(arg) {
            Some(c) => c,
            None => return CommandEffect::Reply(USAGE.into()),
        };
        let ack = ack_for(&cmd);
        let target = app.session.lock().active_view.clone();
        app.session.send_control(target, cmd).await;
        CommandEffect::Reply(ack)
    }
}

fn ack_for(cmd: &ControlCommand) -> String {
    match cmd {
        ControlCommand::GoalCreate { objective } => format!("Goal set: \"{objective}\""),
        ControlCommand::GoalUserPause => "Goal paused.".into(),
        ControlCommand::GoalUserResume => "Goal resumed.".into(),
        ControlCommand::GoalUserComplete => "Goal marked complete.".into(),
        ControlCommand::GoalUserReopen => "Goal reopened.".into(),
        ControlCommand::GoalClear => "Goal cleared.".into(),
        _ => "Goal command sent.".into(),
    }
}

pub(crate) fn parse_goal_arg(arg: &str) -> Option<ControlCommand> {
    if arg.is_empty() {
        return None;
    }
    let lower = arg.to_lowercase();
    match lower.as_str() {
        "pause" => return Some(ControlCommand::GoalUserPause),
        "resume" => return Some(ControlCommand::GoalUserResume),
        "complete" => return Some(ControlCommand::GoalUserComplete),
        "reopen" => return Some(ControlCommand::GoalUserReopen),
        "clear" => return Some(ControlCommand::GoalClear),
        _ => {}
    }
    Some(ControlCommand::GoalCreate {
        objective: arg.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbg_variant(c: &ControlCommand) -> String {
        format!("{c:?}")
    }

    #[test]
    fn empty_arg_returns_none() {
        assert!(parse_goal_arg("").is_none());
    }

    #[test]
    fn lifecycle_keywords_parse() {
        for (input, want) in [
            ("pause", "GoalUserPause"),
            ("RESUME", "GoalUserResume"),
            ("Complete", "GoalUserComplete"),
            ("reopen", "GoalUserReopen"),
            ("clear", "GoalClear"),
        ] {
            let got = dbg_variant(&parse_goal_arg(input).unwrap());
            assert!(got.starts_with(want), "{input} -> {got}");
        }
    }

    #[test]
    fn objective_creates_goal() {
        match parse_goal_arg("ship the goal feature") {
            Some(ControlCommand::GoalCreate { objective }) => {
                assert_eq!(objective, "ship the goal feature");
            }
            other => panic!("expected GoalCreate, got {other:?}"),
        }
    }

    #[test]
    fn ack_strings_per_variant() {
        let mk = |o: &str| ControlCommand::GoalCreate {
            objective: o.into(),
        };
        assert!(ack_for(&mk("x")).contains("\"x\""));
        assert_eq!(ack_for(&ControlCommand::GoalUserPause), "Goal paused.");
        assert_eq!(ack_for(&ControlCommand::GoalUserResume), "Goal resumed.");
        assert_eq!(
            ack_for(&ControlCommand::GoalUserComplete),
            "Goal marked complete."
        );
        assert_eq!(ack_for(&ControlCommand::GoalUserReopen), "Goal reopened.");
        assert_eq!(ack_for(&ControlCommand::GoalClear), "Goal cleared.");
    }

    #[test]
    fn handler_declares_argument() {
        assert!(GoalCmd.has_arg());
    }
}
