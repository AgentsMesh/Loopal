//! Background task log viewer — full-screen output display for a single task.

use loopal_protocol::{BgTaskDetail, BgTaskStatus};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::BgTaskLogState;

pub fn render_bg_task_log(
    f: &mut Frame,
    state: &mut BgTaskLogState,
    details: &[BgTaskDetail],
    area: Rect,
) {
    f.render_widget(Clear, area);

    let task = details.iter().find(|t| t.id == state.task_id);
    let (status, output) = match task {
        Some(t) => (t.status, t.output.as_str()),
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} (not found) ", state.task_id))
                .border_style(Style::default().fg(Color::Red));
            f.render_widget(block, area);
            return;
        }
    };

    let status_label = match status {
        BgTaskStatus::Running => ("Running", Color::Yellow),
        BgTaskStatus::Completed => ("Done", Color::Green),
        BgTaskStatus::Failed => ("Failed", Color::Red),
    };
    let follow_label = if state.auto_follow { " [follow]" } else { "" };
    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(&state.task_id, Style::default().fg(Color::Cyan).bold()),
        Span::styled(" \u{2022} ", Style::default().fg(Color::DarkGray)),
        Span::styled(status_label.0, Style::default().fg(status_label.1)),
        Span::styled(follow_label, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
    ]);

    let hint_bar = build_hint_bar(state.auto_follow);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title_line)
        .title_bottom(hint_bar)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    let lines: Vec<Line<'static>> = output.lines().map(|l| Line::from(l.to_string())).collect();
    let total = lines.len();

    if state.auto_follow {
        state.scroll_offset = total.saturating_sub(inner.height as usize);
    }
    let max_scroll = total.saturating_sub(inner.height as usize);
    if state.scroll_offset > max_scroll {
        state.scroll_offset = max_scroll;
    }
    state.prev_line_count = total;

    let scroll = (state.scroll_offset.min(u16::MAX as usize)) as u16;
    let para = Paragraph::new(lines)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));
    f.render_widget(para, inner);
}

fn build_hint_bar(auto_follow: bool) -> Line<'static> {
    let follow_style = if auto_follow {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{2191}/\u{2193}", Style::default().fg(Color::Cyan)),
        Span::raw(" scroll  "),
        Span::styled("f", follow_style),
        Span::raw(" follow  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" close "),
    ])
}
