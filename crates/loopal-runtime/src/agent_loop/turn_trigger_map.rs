use loopal_protocol::{Envelope, MessageSource};
use loopal_turn::TurnTrigger;

pub fn envelope_to_trigger(env: &Envelope) -> TurnTrigger {
    let envelope_id = env.id.to_string();
    let content = env.content.text.clone();
    match &env.source {
        MessageSource::Human => TurnTrigger::UserInput {
            envelope_id,
            content,
        },
        MessageSource::Scheduled => TurnTrigger::Cron {
            task_id: envelope_id,
            prompt: content,
        },
        MessageSource::Agent(_) | MessageSource::Channel { .. } => TurnTrigger::UserInput {
            envelope_id,
            content,
        },
        MessageSource::System(kind) => {
            // reason: System(...) 覆盖 goal_continuation / background hook / governance
            // 各种触发；用 kind 字符串走分支让命名稳定，不依赖 protocol crate 内部演化。
            match kind.as_str() {
                "goal_continuation" => TurnTrigger::GoalContinuation {
                    goal_id: envelope_id,
                },
                other => TurnTrigger::BackgroundHook {
                    hook_id: other.to_string(),
                    payload: serde_json::json!({
                        "envelope_id": envelope_id,
                        "content": content,
                    }),
                },
            }
        }
    }
}
