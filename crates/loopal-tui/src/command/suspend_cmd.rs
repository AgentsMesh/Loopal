use async_trait::async_trait;
use loopal_protocol::ControlCommand;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

pub struct SuspendCmd;

#[async_trait]
impl CommandHandler for SuspendCmd {
    fn name(&self) -> &str {
        "/suspend"
    }

    fn description(&self) -> &str {
        "Suspend the session: stop cron, goal_continuation, and hook auto-wakes. Only human input resumes."
    }

    fn has_arg(&self) -> bool {
        false
    }

    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let target = app.session.lock().active_view.clone();
        app.session
            .send_control(target, ControlCommand::Suspend)
            .await;
        CommandEffect::Reply(
            "Session suspended. Cron, continuation, and hook wakes are blocked until /resume \
             (or until you send a new message)."
                .into(),
        )
    }
}

pub struct UnsuspendCmd;

#[async_trait]
impl CommandHandler for UnsuspendCmd {
    fn name(&self) -> &str {
        "/unsuspend"
    }

    fn description(&self) -> &str {
        "Lift a /suspend: reopen the continuation gate and allow cron/hook wakes again. \
         (Sending any new message also lifts /suspend automatically.)"
    }

    fn has_arg(&self) -> bool {
        false
    }

    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let target = app.session.lock().active_view.clone();
        app.session
            .send_control(target, ControlCommand::Unsuspend)
            .await;
        CommandEffect::Reply("Session unsuspended.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspend_name_and_description() {
        assert_eq!(SuspendCmd.name(), "/suspend");
        assert!(!SuspendCmd.has_arg());
        assert!(SuspendCmd.description().contains("cron"));
    }

    #[test]
    fn unsuspend_name_and_description() {
        assert_eq!(UnsuspendCmd.name(), "/unsuspend");
        assert!(!UnsuspendCmd.has_arg());
        assert!(
            UnsuspendCmd
                .description()
                .contains("Sending any new message")
        );
    }
}
