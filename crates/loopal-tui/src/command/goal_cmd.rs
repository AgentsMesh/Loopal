use async_trait::async_trait;
use loopal_protocol::ControlCommand;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

pub struct GoalCmd;

#[async_trait]
impl CommandHandler for GoalCmd {
    fn name(&self) -> &str {
        "/goal"
    }

    fn description(&self) -> &str {
        "Manage thread goal: /goal <objective> | pause | resume | complete | clear | extend <N>"
    }

    async fn execute(&self, app: &mut App, arg: Option<&str>) -> CommandEffect {
        let arg = arg.unwrap_or("").trim();
        let cmd = match parse_goal_arg(arg) {
            Some(c) => c,
            None => {
                tracing::warn!(
                    "/goal usage: <objective> | pause | resume | complete | clear | extend <N>"
                );
                return CommandEffect::Done;
            }
        };
        let target = app.session.lock().active_view.clone();
        app.session.send_control(target, cmd).await;
        CommandEffect::Done
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

    #[test]
    fn empty_arg_returns_none() {
        assert!(parse_goal_arg("").is_none());
    }

    #[test]
    fn lifecycle_keywords_parse() {
        assert!(matches!(
            parse_goal_arg("pause"),
            Some(ControlCommand::GoalUserPause)
        ));
        assert!(matches!(
            parse_goal_arg("RESUME"),
            Some(ControlCommand::GoalUserResume)
        ));
        assert!(matches!(
            parse_goal_arg("Complete"),
            Some(ControlCommand::GoalUserComplete)
        ));
        assert!(matches!(
            parse_goal_arg("clear"),
            Some(ControlCommand::GoalClear)
        ));
    }

    #[test]
    fn extend_parses_token_amount() {
        match parse_goal_arg("extend 5000") {
            Some(ControlCommand::GoalExtendBudget { additional_tokens }) => {
                assert_eq!(additional_tokens, 5000);
            }
            other => panic!("expected GoalExtendBudget, got {other:?}"),
        }
    }

    #[test]
    fn extend_with_non_numeric_falls_through_to_objective() {
        match parse_goal_arg("extend the timeout") {
            Some(ControlCommand::GoalCreate {
                objective,
                token_budget,
            }) => {
                assert_eq!(objective, "extend the timeout");
                assert!(token_budget.is_none());
            }
            other => panic!("expected GoalCreate fallthrough, got {other:?}"),
        }
    }

    #[test]
    fn extend_with_only_keyword_falls_through_to_objective() {
        match parse_goal_arg("extend") {
            Some(ControlCommand::GoalCreate { objective, .. }) => {
                assert_eq!(objective, "extend");
            }
            other => panic!("expected GoalCreate, got {other:?}"),
        }
    }

    #[test]
    fn plain_objective_creates_goal() {
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
    }

    #[test]
    fn objective_with_budget_flag_parses_both() {
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
}
