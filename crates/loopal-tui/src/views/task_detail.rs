use loopal_protocol::{TaskSnapshot, TaskSnapshotStatus};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::TaskDetailState;

pub fn render_task_detail(
    f: &mut Frame,
    state: &mut TaskDetailState,
    tasks: &[TaskSnapshot],
    area: Rect,
) {
    f.render_widget(Clear, area);
    let Some(task) = tasks.iter().find(|t| t.id == state.task_id) else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" #{} (not found) ", state.task_id))
            .border_style(Style::default().fg(Color::Red));
        f.render_widget(block, area);
        return;
    };

    let (status_label, status_color) = match task.status {
        TaskSnapshotStatus::Pending => ("Pending", Color::Yellow),
        TaskSnapshotStatus::InProgress => ("In Progress", Color::Green),
        TaskSnapshotStatus::Completed => ("Completed", Color::Cyan),
    };
    let title = Line::from(vec![
        Span::raw(" #"),
        Span::styled(&task.id, Style::default().fg(Color::Cyan).bold()),
        Span::styled(" \u{2022} ", Style::default().fg(Color::DarkGray)),
        Span::styled(status_label, Style::default().fg(status_color)),
        Span::raw(" "),
    ]);
    let hint = Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{2191}/\u{2193}", Style::default().fg(Color::Cyan)),
        Span::raw(" scroll  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" close "),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_bottom(hint)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 1 {
        return;
    }

    let lines = build_lines(task);
    let max_scroll = lines.len().saturating_sub(inner.height as usize);
    if state.scroll_offset > max_scroll {
        state.scroll_offset = max_scroll;
    }
    let scroll = (state.scroll_offset.min(u16::MAX as usize)) as u16;
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn build_lines(task: &TaskSnapshot) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Subject:",
        Style::default().fg(Color::Yellow).bold(),
    )));
    lines.push(Line::from(Span::styled(
        task.subject.clone(),
        Style::default().fg(Color::White),
    )));
    if let Some(ref af) = task.active_form {
        lines.push(Line::raw(""));
        lines.push(field("Active form", af));
    }
    if !task.blocked_by.is_empty() {
        lines.push(Line::raw(""));
        lines.push(field("Blocked by", &task.blocked_by.join(", ")));
    }
    if !task.blocks.is_empty() {
        lines.push(field("Blocks", &task.blocks.join(", ")));
    }
    if !task.description.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Description:",
            Style::default().fg(Color::Yellow).bold(),
        )));
        for line in task.description.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )));
        }
    }
    lines
}

fn field(name: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {name:<12}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}
