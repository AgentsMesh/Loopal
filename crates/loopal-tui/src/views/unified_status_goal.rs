use ratatui::prelude::*;

use loopal_protocol::{ThreadGoal, ThreadGoalStatus};

pub(super) fn append_goal_indicator(spans: &mut Vec<Span<'static>>, goal: &ThreadGoal) {
    let (label, color) = match goal.status {
        ThreadGoalStatus::Active => ("active", Color::Cyan),
        ThreadGoalStatus::Paused => ("paused", Color::Yellow),
        ThreadGoalStatus::Complete => ("done", Color::Green),
        ThreadGoalStatus::Infeasible => ("infeasible", Color::Red),
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
