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
            envelope_id,
            content,
        },
        MessageSource::Agent(addr) => TurnTrigger::Agent {
            envelope_id,
            from: addr.to_string(),
            content,
        },
        MessageSource::Channel { channel, from } => TurnTrigger::Channel {
            envelope_id,
            channel: channel.clone(),
            from: from.to_string(),
            content,
        },
        MessageSource::System(kind) => {
            // reason: System(...) 覆盖 goal_continuation 与各种 background hook；
            // goal_continuation 是已知的特例，其他 kind 都归入 BackgroundHook。
            if kind == "goal_continuation" {
                TurnTrigger::GoalContinuation {
                    envelope_id,
                    content,
                }
            } else {
                TurnTrigger::BackgroundHook {
                    envelope_id,
                    hook_kind: kind.clone(),
                    content,
                }
            }
        }
    }
}
