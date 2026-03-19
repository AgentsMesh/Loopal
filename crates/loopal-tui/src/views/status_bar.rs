use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;

/// Render the status bar at the bottom of the screen.
/// In plan mode, uses magenta background with read-only indicator.
pub fn render_status_bar(f: &mut Frame, state: &SessionState, area: Rect) {
    let is_plan = state.mode == "plan";
    let mode_style = if is_plan {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::Green).bold()
    };

    let token_count = state.token_count();
    let context_info = if state.context_window > 0 {
        format!(
            "ctx: {}k/{}k",
            token_count / 1000,
            state.context_window / 1000
        )
    } else {
        format!("tokens: {}", token_count)
    };

    let mut spans = vec![
        Span::styled(format!(" {} ", state.mode.to_uppercase()), mode_style),
    ];
    if is_plan {
        spans.push(Span::styled(
            " read-only ",
            Style::default().fg(Color::Magenta),
        ));
    }
    spans.push(Span::raw(" | "));
    spans.push(Span::styled(&state.model, Style::default().fg(Color::Cyan)));
    spans.push(Span::raw(" | "));
    spans.push(Span::raw(context_info));

    // Show cache hit rate when cache reads are present
    if state.cache_read_tokens > 0 {
        let total_cache = state.cache_creation_tokens + state.cache_read_tokens;
        let hit_pct = (state.cache_read_tokens as f64 / total_cache as f64 * 100.0) as u32;
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            format!("cache: {}%", hit_pct),
            Style::default().fg(Color::Green),
        ));
    }

    spans.push(Span::raw(" | "));
    spans.push(Span::raw(format!("turns: {}", state.turn_count)));
    if !state.inbox.is_empty() {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            format!("inbox: {}", state.inbox.len()),
            Style::default().fg(Color::Yellow),
        ));
    }

    let bg = if is_plan {
        Style::default().bg(Color::Rgb(50, 20, 50))
    } else {
        Style::default().bg(Color::DarkGray)
    };

    let paragraph = Paragraph::new(Line::from(spans)).style(bg);
    f.render_widget(paragraph, area);
}
