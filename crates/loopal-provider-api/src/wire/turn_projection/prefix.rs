use loopal_turn::TurnTrigger;

/// SSOT for the LLM-facing text of a turn trigger: applies the source prefix
/// (`[scheduled]`, `[from: ...]`) or `<agent-result>` marker. Returns `None`
/// for triggers that produce no user message (`Resume`). Images on
/// `UserInput` are structural and handled by the caller — this is text only.
pub fn trigger_llm_text(trigger: &TurnTrigger) -> Option<String> {
    match trigger {
        TurnTrigger::UserInput { content, .. } => Some(content.clone()),
        TurnTrigger::Cron { content, .. } => Some(format!("[scheduled] {content}")),
        TurnTrigger::Agent { from, content, .. } => Some(format!("[from: {from}] {content}")),
        TurnTrigger::AgentResult { from, content, .. } => Some(format!(
            "<agent-result name=\"{from}\">\n{content}\n</agent-result>"
        )),
        TurnTrigger::Channel {
            channel,
            from,
            content,
            ..
        } => Some(format!("[from: #{channel}/{from}] {content}")),
        TurnTrigger::GoalContinuation { content, .. } => Some(content.clone()),
        TurnTrigger::BackgroundHook { content, .. } => Some(content.clone()),
        TurnTrigger::Resume => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_result(from: &str, content: &str) -> TurnTrigger {
        TurnTrigger::AgentResult {
            envelope_id: String::new(),
            from: from.into(),
            content: content.into(),
        }
    }

    #[test]
    fn agent_result_wraps_in_marker() {
        assert_eq!(
            trigger_llm_text(&agent_result("worker", "done")).unwrap(),
            "<agent-result name=\"worker\">\ndone\n</agent-result>"
        );
    }

    #[test]
    fn cron_and_agent_carry_source_prefix() {
        let cron = TurnTrigger::Cron {
            envelope_id: String::new(),
            content: "tick".into(),
        };
        assert_eq!(trigger_llm_text(&cron).unwrap(), "[scheduled] tick");
        let agent = TurnTrigger::Agent {
            envelope_id: String::new(),
            from: "hub/w".into(),
            content: "hi".into(),
        };
        assert_eq!(trigger_llm_text(&agent).unwrap(), "[from: hub/w] hi");
    }

    #[test]
    fn resume_produces_no_text() {
        assert_eq!(trigger_llm_text(&TurnTrigger::Resume), None);
    }
}
