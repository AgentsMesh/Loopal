//! Background shell tasks panel — shows running background processes.
//!
//! Rendered below the agent panel. Each running task = 1 line:
//! ```text
//!  ▸ ⠹ bg_3  (auto-bg) npm install
//!    ⠧ bg_5  (auto-bg) cargo build --release
//! ```
//!
//! The panel reads from `App.bg_snapshots` (protocol types) instead of
//! accessing the tool-level store directly.

use loopal_protocol::{BgTaskSnapshot, BgTaskStatus};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::unified_status::spinner_frame;

/// Maximum background task lines to show.
const MAX_BG_VISIBLE: usize = 3;

/// Height needed for the background tasks sub-panel.
///
/// Returns 0 when no running background tasks exist.
pub fn bg_panel_height(snapshots: &[BgTaskSnapshot]) -> u16 {
    let count = snapshots.len();
    if count == 0 {
        return 0;
    }
    count.min(MAX_BG_VISIBLE) as u16
}

/// IDs of all background tasks from the cached snapshots.
///
/// Used by `panel_ops` for focus cycling.
pub fn task_ids(snapshots: &[BgTaskSnapshot]) -> Vec<String> {
    snapshots.iter().map(|s| s.id.clone()).collect()
}

/// Render background task lines into the given area.
pub fn render_bg_tasks(
    f: &mut Frame,
    snapshots: &[BgTaskSnapshot],
    focused_task: Option<&str>,
    elapsed: std::time::Duration,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }
    let lines: Vec<Line<'static>> = snapshots
        .iter()
        .take(MAX_BG_VISIBLE)
        .map(|t| render_task_line(t, focused_task, elapsed))
        .collect();

    let bg = Style::default().bg(Color::Rgb(25, 25, 30));
    f.render_widget(Paragraph::new(lines).style(bg), area);
}

fn render_task_line(
    task: &BgTaskSnapshot,
    focused: Option<&str>,
    elapsed: std::time::Duration,
) -> Line<'static> {
    let is_focused = focused == Some(task.id.as_str());
    let indicator = if is_focused { " ▸ " } else { "   " };
    let indicator_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default()
    };

    let (icon, icon_style) = match task.status {
        BgTaskStatus::Running => (
            spinner_frame(elapsed).to_string(),
            Style::default().fg(Color::Yellow),
        ),
        BgTaskStatus::Completed => ("✓".into(), Style::default().fg(Color::Green)),
        BgTaskStatus::Failed => ("✗".into(), Style::default().fg(Color::Red)),
    };
    let desc: String = task.description.chars().take(40).collect();
    let id_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Line::from(vec![
        Span::styled(indicator.to_string(), indicator_style),
        Span::styled(icon, icon_style),
        Span::raw(" "),
        Span::styled(format!("{:<6}", task.id), id_style),
        Span::raw(" "),
        Span::styled(desc, Style::default().fg(Color::Rgb(100, 100, 100))),
    ])
}
