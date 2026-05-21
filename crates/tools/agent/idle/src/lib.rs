use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolDispatch, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub const MIN_IDLE_DURATION_SECS: u64 = 60;
pub const MAX_IDLE_DURATION_SECS: u64 = 86_400;

pub struct RequestIdleTool;

#[derive(Deserialize, JsonSchema)]
pub struct RequestIdleParams {
    pub reason: String,
    pub max_idle_duration_secs: u64,
    #[serde(default)]
    pub expected_wake_signal: Option<String>,
}

pub fn validate_duration(secs: u64) -> Result<(), String> {
    if secs < MIN_IDLE_DURATION_SECS {
        return Err(format!(
            "max_idle_duration_secs={secs} is below the minimum of {MIN_IDLE_DURATION_SECS} \
             (1 minute). Set a deadline you are confident is the soonest you might need to act \
             again; the runtime auto-reopens at that point."
        ));
    }
    if secs > MAX_IDLE_DURATION_SECS {
        return Err(format!(
            "max_idle_duration_secs={secs} exceeds the maximum of {MAX_IDLE_DURATION_SECS} \
             (24 hours). If you need a longer pause, call `update_goal` with status \
             `infeasible` instead so the user is notified."
        ));
    }
    Ok(())
}

#[async_trait]
impl TypedTool<RequestIdleParams> for RequestIdleTool {
    fn name(&self) -> &str {
        "request_idle"
    }

    fn description(&self) -> &str {
        "Declare that you have no productive next action and want the runtime to stop \
         injecting `goal_continuation` prompts until either (a) the deadline you set \
         elapses, or (b) any external signal arrives (user message, cron fire, background \
         hook). Use this when you have done what you can and need to wait for outside input \
         instead of generating filler text. Does NOT change the goal status — the goal \
         remains `active`. Parameters: `reason` (30-500 chars, natural-language note for \
         the audit trail), `max_idle_duration_secs` (required, 60..=86400; runtime auto-\
         reopens the gate after this elapses so you cannot get stuck idle forever), and \
         `expected_wake_signal` (optional self-describing string like \"next cron fire\" \
         — informational only)."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::RunnerDirect
    }

    async fn execute(
        &self,
        _input: RequestIdleParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success(
            "idle acknowledged (intercepted by runner)",
        ))
    }
}
