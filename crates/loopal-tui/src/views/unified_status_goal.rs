use ratatui::prelude::*;

use loopal_protocol::{ThreadGoal, ThreadGoalStatus};

use super::unified_status::dim_style;

pub(super) fn append_goal_indicator(spans: &mut Vec<Span<'static>>, goal: &ThreadGoal) {
    let (label, color) = match goal.status {
        ThreadGoalStatus::Active => ("active", Color::Cyan),
        ThreadGoalStatus::Paused => ("paused", Color::Yellow),
        ThreadGoalStatus::BudgetLimited => ("budget", Color::Red),
        ThreadGoalStatus::Complete => ("done", Color::Green),
    };
    spans.push(Span::styled("◆ ", Style::default().fg(color).bold()));
    spans.push(Span::styled(
        truncate_objective(&goal.objective),
        Style::default().fg(color),
    ));
    spans.push(Span::styled(
        format!(" [{label}]"),
        Style::default().fg(color).bold(),
    ));
    if let Some(b) = goal.token_budget {
        let used_k = goal.tokens_used / 1000;
        let budget_k = b / 1000;
        let usage_color = if goal.budget_exhausted() {
            Color::Red
        } else {
            Color::DarkGray
        };
        spans.push(Span::styled(
            format!(" {used_k}k/{budget_k}k"),
            Style::default().fg(usage_color),
        ));
    } else if goal.tokens_used > 0 {
        spans.push(Span::styled(
            format!(" {}k", goal.tokens_used / 1000),
            dim_style(),
        ));
    }
}

fn truncate_objective(s: &str) -> String {
    const MAX: usize = 28;
    let trimmed: String = s.chars().take(MAX).collect();
    if s.chars().count() > MAX {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
