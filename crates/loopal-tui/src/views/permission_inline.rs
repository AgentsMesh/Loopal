use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_view_state::PendingPermission;

const MAX_JSON_LINES: usize = 6;
const MAX_HEIGHT: u16 = 12;

pub struct Prepared {
    pub name: String,
    pub json_lines: Vec<String>,
    pub total_lines: usize,
}

pub fn prepare(p: &PendingPermission) -> Prepared {
    let json = serde_json::to_string_pretty(&p.input).unwrap_or_else(|_| p.input.to_string());
    let total_lines = json.lines().count();
    let json_lines: Vec<String> = json
        .lines()
        .take(MAX_JSON_LINES)
        .map(String::from)
        .collect();
    Prepared {
        name: p.name.clone(),
        json_lines,
        total_lines,
    }
}

pub fn height_of(prepared: &Prepared) -> u16 {
    let json_h = prepared.json_lines.len() as u16
        + u16::from(prepared.total_lines > prepared.json_lines.len());
    (1 + json_h + 1).min(MAX_HEIGHT)
}

pub fn height(p: &PendingPermission, _width: u16) -> u16 {
    height_of(&prepare(p))
}

pub fn render_prepared(f: &mut Frame, prepared: &Prepared, area: Rect, status: Option<&str>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let title = Line::from(Span::styled(
        format!("⚠ Tool: {}", prepared.name),
        Style::default().fg(Color::Yellow).bold(),
    ));
    let hint = if let Some(s) = status {
        Line::from(Span::styled(
            format!("⚠ {s}"),
            Style::default().fg(Color::Yellow).bold(),
        ))
    } else {
        Line::from(vec![
            Span::styled("[y] Allow  ", Style::default().fg(Color::Green).bold()),
            Span::styled("[n] Deny  ", Style::default().fg(Color::Red).bold()),
            Span::styled("Esc Cancel", Style::default().fg(Color::DarkGray).italic()),
        ])
    };

    let cap = area.height as usize;
    let mut lines: Vec<Line> = Vec::with_capacity(cap);
    lines.push(title);

    let json_budget = cap.saturating_sub(2);
    let mut shown = 0usize;
    for line in prepared.json_lines.iter().take(json_budget) {
        lines.push(Line::from(Span::styled(
            format!("    {line}"),
            Style::default().fg(Color::Gray),
        )));
        shown += 1;
    }
    let extra = prepared.total_lines.saturating_sub(shown);
    if extra > 0 && shown < json_budget {
        lines.push(Line::from(Span::styled(
            format!("    … ({extra} more lines)"),
            Style::default().fg(Color::DarkGray),
        )));
    }
    if cap >= 2 {
        lines.push(hint);
    }

    f.render_widget(Paragraph::new(lines), area);
}
