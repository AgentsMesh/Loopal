use loopal_tool_api::GoalSessionError;

pub fn format_session_error(err: GoalSessionError) -> String {
    match err {
        GoalSessionError::AlreadyExists => {
            "this thread already has a goal; use update_goal only when the existing goal is complete"
                .to_string()
        }
        GoalSessionError::NotFound => "no goal exists for this thread".to_string(),
        GoalSessionError::ModelStatusForbidden => {
            "update_goal can only mark the existing goal complete; pause, resume and budget changes are user-controlled"
                .to_string()
        }
        GoalSessionError::InvalidBudget => {
            "token_budget must be a positive integer when provided".to_string()
        }
        GoalSessionError::ObjectiveTooLong { max, got } => {
            format!("objective must be 1..={max} characters; got {got}")
        }
        GoalSessionError::Storage(s) => format!("goal storage error: {s}"),
    }
}
