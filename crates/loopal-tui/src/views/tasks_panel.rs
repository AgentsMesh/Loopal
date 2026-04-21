//! Structured tasks panel — shows TaskCreate/TaskUpdate progress.
//!
//! Rendered in the panel zone alongside agents and background tasks.
//! ```text
//!  ▸ ✓ #1  Fix the bug
//!    ⠹ #2  Add tests                  Adding tests
//!    ○ #3  Update docs                [blocked]
//! ```

use loopal_protocol::{TaskSnapshot, TaskSnapshotStatus};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::text_width::truncate_to_width;
use super::unified_status::spinner_frame;

pub const MAX_TASK_VISIBLE: usize = 5;

pub fn tasks_panel_height(snapshots: &[TaskSnapshot]) -> u16 {
    let count = active_count(snapshots);
    if count == 0 {
        return 0;
    }
    count.min(MAX_TASK_VISIBLE) as u16
}

pub fn task_ids(snapshots: &[TaskSnapshot]) -> Vec<String> {
    snapshots
        .iter()
        .filter(|t| !matches!(t.status, TaskSnapshotStatus::Completed))
        .map(|t| t.id.clone())
        .collect()
}

pub fn render_tasks_panel(
    f: &mut Frame,
    snapshots: &[TaskSnapshot],
    focused: Option<&str>,
    elapsed: std::time::Duration,
    offset: usize,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }
    let active: Vec<_> = snapshots
        .iter()
        .filter(|t| !matches!(t.status, TaskSnapshotStatus::Completed))
        .collect();
    let total = active.len();
    let clamped = offset.min(total.saturating_sub(MAX_TASK_VISIBLE));
    let end = (clamped + MAX_TASK_VISIBLE).min(total);
    let lines: Vec<Line<'static>> = active[clamped..end]
        .iter()
        .map(|t| render_task_line(t, focused, elapsed, area.width as usize))
        .collect();

    let bg = Style::default().bg(Color::Rgb(25, 25, 30));
    f.render_widget(Paragraph::new(lines).style(bg), area);
}

fn render_task_line(
    task: &TaskSnapshot,
    focused: Option<&str>,
    elapsed: std::time::Duration,
    width: usize,
) -> Line<'static> {
    let is_focused = focused == Some(task.id.as_str());
    let indicator = if is_focused { " ▸ " } else { "   " };
    let indicator_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default()
    };
    let (icon, icon_style) = status_icon(&task.status, &task.blocked_by, elapsed);
    let id_label = format!("#{:<3}", task.id);
    let id_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Subject: truncate to fit (by terminal width, not byte length).
    let prefix_len = 3 + 2 + 5; // indicator + icon+space + id
    let suffix = task_suffix(task);
    let suffix_len = suffix.len();
    let max_subject = width.saturating_sub(prefix_len + suffix_len + 2);
    let (subject, subject_width) = truncate_to_width(&task.subject, max_subject);
    let pad = max_subject.saturating_sub(subject_width);

    let mut spans = vec![
        Span::styled(indicator.to_string(), indicator_style),
        Span::styled(icon, icon_style),
        Span::raw(" "),
        Span::styled(id_label, id_style),
        Span::styled(subject, Style::default().fg(Color::White)),
        Span::raw(" ".repeat(pad)),
    ];
    if !suffix.is_empty() {
        spans.push(Span::styled(
            format!("  {suffix}"),
            Style::default().fg(Color::Rgb(80, 80, 80)),
        ));
    }
    Line::from(spans)
}

fn status_icon(
    status: &TaskSnapshotStatus,
    blocked_by: &[String],
    elapsed: std::time::Duration,
) -> (String, Style) {
    match status {
        TaskSnapshotStatus::Completed => ("✓".into(), Style::default().fg(Color::Green)),
        TaskSnapshotStatus::InProgress => (
            spinner_frame(elapsed).to_string(),
            Style::default().fg(Color::Green),
        ),
        TaskSnapshotStatus::Pending if !blocked_by.is_empty() => {
            ("◌".into(), Style::default().fg(Color::DarkGray))
        }
        TaskSnapshotStatus::Pending => ("○".into(), Style::default().fg(Color::Yellow)),
    }
}

fn task_suffix(task: &TaskSnapshot) -> String {
    if task.status == TaskSnapshotStatus::InProgress
        && let Some(ref af) = task.active_form
    {
        return af.clone();
    }
    if !task.blocked_by.is_empty() {
        return "[blocked]".into();
    }
    String::new()
}

/// Non-completed task count — used by the panel height calc, section
/// headers, and focus tracking. Allocation-free alternative to
/// `task_ids(...).len()`.
pub fn active_count(snapshots: &[TaskSnapshot]) -> usize {
    snapshots
        .iter()
        .filter(|t| !matches!(t.status, TaskSnapshotStatus::Completed))
        .count()
}
