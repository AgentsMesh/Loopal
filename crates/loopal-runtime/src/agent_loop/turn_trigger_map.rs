use loopal_protocol::{Envelope, MessageSource};
use loopal_tool_invocation::ToolImageBlock;
use loopal_turn::TurnTrigger;

pub fn envelope_to_trigger(env: &Envelope) -> TurnTrigger {
    let envelope_id = env.id.to_string();
    let content = env.content.text.clone();
    match &env.source {
        MessageSource::Human => TurnTrigger::UserInput {
            envelope_id,
            content,
            images: env
                .content
                .images
                .iter()
                .map(|img| ToolImageBlock::Inline {
                    media_type: img.media_type.clone(),
                    data: img.data.clone(),
                })
                .collect(),
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
        MessageSource::AgentResult { child } => TurnTrigger::AgentResult {
            envelope_id,
            // reason: bare agent name (not child.to_string()) — the
            // <agent-result name=...> marker must stay hub-agnostic. After
            // uplink SNAT `child` carries a hub path; to_string() would leak
            // it into the marker the parent LLM reads.
            from: child.agent.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_protocol::QualifiedAddress;

    #[test]
    fn agent_result_source_maps_to_bare_child_name() {
        let env = Envelope::new(
            MessageSource::AgentResult {
                child: QualifiedAddress::remote(["hub-a"], "worker"),
            },
            "parent",
            "done",
        );
        match envelope_to_trigger(&env) {
            TurnTrigger::AgentResult { from, content, .. } => {
                // Hub path is stripped: marker stays hub-agnostic even for
                // cross-hub completions whose source was SNAT-stamped.
                assert_eq!(from, "worker");
                assert_eq!(content, "done");
            }
            other => panic!("expected AgentResult trigger, got {other:?}"),
        }
    }
}
