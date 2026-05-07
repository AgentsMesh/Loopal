use async_trait::async_trait;
use loopal_protocol::ControlCommand;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

const USAGE: &str =
    "/goal usage: <objective> [--budget=<N>] | pause | resume | complete | clear | extend <N>";

pub struct GoalCmd;

#[async_trait]
impl CommandHandler for GoalCmd {
    fn name(&self) -> &str {
        "/goal"
    }

    fn description(&self) -> &str {
        "Manage thread goal: /goal <objective> | pause | resume | complete | clear | extend <N>"
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
        ControlCommand::GoalCreate {
            objective,
            token_budget,
        } => match token_budget {
            Some(b) => format!("Goal set: \"{objective}\" (budget {b} tokens)"),
            None => format!("Goal set: \"{objective}\""),
        },
        ControlCommand::GoalUserPause => "Goal paused.".into(),
        ControlCommand::GoalUserResume => "Goal resumed.".into(),
        ControlCommand::GoalUserComplete => "Goal marked complete.".into(),
        ControlCommand::GoalClear => "Goal cleared.".into(),
        ControlCommand::GoalExtendBudget { additional_tokens } => {
            format!("Goal budget extended by {additional_tokens} tokens.")
        }
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
        "clear" => return Some(ControlCommand::GoalClear),
        _ => {}
    }
    // reason: only treat "extend N" as the budget-extend command when the
    // remainder parses as a positive integer. Otherwise the input is a
    // perfectly valid objective like "extend the timeout" and must fall
    // through to the GoalCreate path below.
    if let Some(rest) = lower.strip_prefix("extend ")
        && let Ok(n) = rest.trim().parse::<u64>()
    {
        return Some(ControlCommand::GoalExtendBudget {
            additional_tokens: n,
        });
    }
    if let Some(budget_part) = arg.find(" --budget=") {
        let objective = arg[..budget_part].trim().to_string();
        let budget_str = arg[budget_part + 10..].trim();
        let token_budget = budget_str.parse::<u64>().ok();
        if !objective.is_empty() && token_budget.is_some() {
            return Some(ControlCommand::GoalCreate {
                objective,
                token_budget,
            });
        }
    }
    Some(ControlCommand::GoalCreate {
        objective: arg.to_string(),
        token_budget: None,
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
            ("clear", "GoalClear"),
        ] {
            let got = dbg_variant(&parse_goal_arg(input).unwrap());
            assert!(got.starts_with(want), "{input} -> {got}");
        }
    }

    #[test]
    fn extend_parses_token_amount_else_falls_through() {
        match parse_goal_arg("extend 5000") {
            Some(ControlCommand::GoalExtendBudget { additional_tokens }) => {
                assert_eq!(additional_tokens, 5000)
            }
            other => panic!("expected GoalExtendBudget, got {other:?}"),
        }
        match parse_goal_arg("extend the timeout") {
            Some(ControlCommand::GoalCreate { objective, .. }) => {
                assert_eq!(objective, "extend the timeout")
            }
            other => panic!("expected GoalCreate fallthrough, got {other:?}"),
        }
    }

    #[test]
    fn objective_creates_goal_with_optional_budget() {
        match parse_goal_arg("ship the goal feature") {
            Some(ControlCommand::GoalCreate {
                objective,
                token_budget,
            }) => {
                assert_eq!(objective, "ship the goal feature");
                assert!(token_budget.is_none());
            }
            other => panic!("expected GoalCreate, got {other:?}"),
        }
        match parse_goal_arg("ship feature --budget=10000") {
            Some(ControlCommand::GoalCreate {
                objective,
                token_budget,
            }) => {
                assert_eq!(objective, "ship feature");
                assert_eq!(token_budget, Some(10_000));
            }
            other => panic!("expected GoalCreate with budget, got {other:?}"),
        }
    }

    #[test]
    fn ack_strings_per_variant() {
        let mk = |o: &str, b: Option<u64>| ControlCommand::GoalCreate {
            objective: o.into(),
            token_budget: b,
        };
        assert!(ack_for(&mk("x", None)).contains("\"x\""));
        assert!(ack_for(&mk("x", Some(42))).contains("42"));
        assert_eq!(ack_for(&ControlCommand::GoalUserPause), "Goal paused.");
        assert_eq!(ack_for(&ControlCommand::GoalUserResume), "Goal resumed.");
        assert_eq!(
            ack_for(&ControlCommand::GoalUserComplete),
            "Goal marked complete."
        );
        assert_eq!(ack_for(&ControlCommand::GoalClear), "Goal cleared.");
        assert!(
            ack_for(&ControlCommand::GoalExtendBudget {
                additional_tokens: 99
            })
            .contains("99")
        );
    }

    #[test]
    fn handler_declares_argument() {
        assert!(GoalCmd.has_arg());
    }
}
