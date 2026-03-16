use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::App;

/// Render the status bar at the bottom of the screen.
pub fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.mode.as_str() {
        "plan" => Style::default().fg(Color::Magenta).bold(),
        _ => Style::default().fg(Color::Green).bold(),
    };

    let context_info = if app.context_window > 0 {
        format!(
            "ctx: {}k/{}k",
            app.token_count / 1000,
            app.context_window / 1000
        )
    } else {
        format!("tokens: {}", app.token_count)
    };

    let mut spans = vec![
        Span::styled(format!(" {} ", app.mode.to_uppercase()), mode_style),
        Span::raw(" | "),
        Span::styled(&app.model, Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::raw(context_info),
        Span::raw(" | "),
        Span::raw(format!("turns: {}", app.turn_count)),
    ];
    if !app.inbox.is_empty() {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            format!("inbox: {}", app.inbox.len()),
            Style::default().fg(Color::Yellow),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(paragraph, area);
}
