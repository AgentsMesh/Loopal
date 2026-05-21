use loopal_protocol::DegenerationSignal;

pub fn build_feedback(signal: DegenerationSignal, count: u32) -> String {
    match signal {
        DegenerationSignal::RepeatedText => format!(
            "Your last {count} turns produced identical text output. The runtime closed the \
             continuation gate. Either change strategy, call `request_idle` to declare \
             idle, or call `update_goal` with status `infeasible` if the objective is \
             structurally unreachable."
        ),
        DegenerationSignal::BarrenStreak => format!(
            "{count} consecutive turns produced no tool calls. The runtime closed the \
             continuation gate as a safety net. If you have nothing actionable, call \
             `request_idle`; if the goal cannot be reached, call `update_goal` with \
             status `infeasible`."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_text_feedback_mentions_request_idle_and_infeasible() {
        let s = build_feedback(DegenerationSignal::RepeatedText, 7);
        assert!(s.contains("7"));
        assert!(s.contains("request_idle"));
        assert!(s.contains("infeasible"));
    }

    #[test]
    fn barren_streak_feedback_mentions_safety_net() {
        let s = build_feedback(DegenerationSignal::BarrenStreak, 20);
        assert!(s.contains("20"));
        assert!(s.contains("safety net"));
        assert!(s.contains("request_idle"));
    }
}
