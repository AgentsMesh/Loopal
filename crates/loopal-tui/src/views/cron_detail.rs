use chrono::{DateTime, Local, TimeZone, Utc};
use loopal_protocol::CronJobSnapshot;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::cron_duration_format::format_next_fire_ms;
use crate::app::CronDetailState;

pub fn render_cron_detail(
    f: &mut Frame,
    state: &CronDetailState,
    crons: &[CronJobSnapshot],
    area: Rect,
) {
    f.render_widget(Clear, area);
    let Some(cron) = crons.iter().find(|c| c.id == state.cron_id) else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} (not found) ", state.cron_id))
            .border_style(Style::default().fg(Color::Red));
        f.render_widget(block, area);
        return;
    };

    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(&cron.id, Style::default().fg(Color::Cyan).bold()),
        Span::styled(" \u{2022} cron ", Style::default().fg(Color::DarkGray)),
        Span::styled(&cron.cron_expr, Style::default().fg(Color::Yellow)),
        Span::raw(" "),
    ]);
    let hint = Line::from(vec![
        Span::raw(" "),
        Span::styled("x", Style::default().fg(Color::Red).bold()),
        Span::raw(" stop  "),
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
    let lines = build_lines(cron);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn build_lines(cron: &CronJobSnapshot) -> Vec<Line<'static>> {
    let recurring = if cron.recurring {
        "yes"
    } else {
        "no (one-shot)"
    };
    let durable = if cron.durable {
        "yes (persisted)"
    } else {
        "no (in-memory)"
    };
    let created = format_local(cron.created_at_unix_ms);
    let now = Utc::now();
    let next_fire = format_next_fire_ms(cron.next_fire_unix_ms, now);
    vec![
        field("recurring", recurring),
        field("durable", durable),
        field("created", &created),
        field("next fire", &next_fire),
        Line::raw(""),
        Line::from(Span::styled(
            "Prompt:",
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::from(Span::styled(
            cron.prompt.clone(),
            Style::default().fg(Color::White),
        )),
    ]
}

fn field(name: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {name:<11}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

fn format_local(unix_ms: i64) -> String {
    if unix_ms == 0 {
        return "unknown".into();
    }
    match Utc.timestamp_millis_opt(unix_ms).single() {
        Some(t) => DateTime::<Local>::from(t)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        None => "invalid".into(),
    }
}
