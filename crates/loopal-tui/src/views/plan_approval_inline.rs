use loopal_view_state::PendingPlanApproval;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub(crate) const CONTENT_ROWS: usize = 7;
const MAX_HEIGHT: u16 = 10;

pub fn height(plan: &PendingPlanApproval, _width: u16) -> u16 {
    let content = plan.plan_content.lines().count().max(1);
    (3 + content.min(CONTENT_ROWS) as u16).min(MAX_HEIGHT)
}

pub fn content_viewport_rows(area_height: u16) -> usize {
    (area_height as usize).saturating_sub(3).min(CONTENT_ROWS)
}

pub fn max_scroll(plan: &PendingPlanApproval, viewport_rows: usize) -> usize {
    plan.plan_content
        .lines()
        .count()
        .saturating_sub(viewport_rows.max(1))
}

pub fn render(
    f: &mut Frame,
    plan: &PendingPlanApproval,
    scroll: usize,
    area: Rect,
    status: Option<&str>,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let cap = area.height as usize;
    let mut lines = Vec::with_capacity(cap);
    lines.push(Line::from(Span::styled(
        "? Review plan",
        Style::default().fg(Color::Yellow).bold(),
    )));
    if cap > 1 {
        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(plan.plan_path.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    let content: Vec<&str> = plan.plan_content.lines().collect();
    let budget = content_viewport_rows(area.height);
    let offset = scroll.min(max_scroll(plan, budget));
    if content.is_empty() && budget > 0 {
        lines.push(Line::from(Span::styled(
            "  (empty plan)",
            Style::default().fg(Color::DarkGray).italic(),
        )));
    } else {
        lines.extend(content.iter().skip(offset).take(budget).map(|line| {
            Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(Color::Gray),
            ))
        }));
    }

    if cap >= 3 {
        lines.push(action_line(status, offset, content.len(), budget));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn action_line(status: Option<&str>, offset: usize, total: usize, shown: usize) -> Line<'static> {
    if let Some(status) = status {
        return Line::from(Span::styled(
            status.to_string(),
            Style::default().fg(Color::Yellow).bold(),
        ));
    }
    let end = (offset + shown).min(total);
    let range = if shown > 0 && total > shown {
        format!(" Lines {}-{end}/{total}  Up/Down scroll ", offset + 1)
    } else {
        String::new()
    };
    Line::from(vec![
        Span::styled(
            " Approve [y] ",
            Style::default().fg(Color::Black).bg(Color::Green).bold(),
        ),
        Span::raw("  "),
        Span::styled(
            " Reject [n] ",
            Style::default().fg(Color::Black).bg(Color::Red).bold(),
        ),
        Span::styled(range, Style::default().fg(Color::DarkGray).italic()),
    ])
}
