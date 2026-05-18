use loopal_tool_api::GoalSessionError;

pub fn format_session_error(err: GoalSessionError) -> String {
    match err {
        GoalSessionError::AlreadyExists => {
            "this thread already has an in-progress goal; if you intend to continue the same \
             objective use update_goal with status `active`, otherwise wait for the user or \
             system to clear the existing goal"
                .to_string()
        }
        GoalSessionError::NotFound => "no goal exists for this thread".to_string(),
        GoalSessionError::ModelStatusForbidden => {
            "update_goal cannot apply that status to the goal's current state; \
             call get_goal to inspect the current status before retrying"
                .to_string()
        }
        GoalSessionError::ObjectiveTooLong { max, got } => {
            format!("objective must be 1..={max} characters; got {got}")
        }
        GoalSessionError::Storage(s) => format!("goal storage error: {s}"),
    }
}
