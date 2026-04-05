//! Status tab — key-value display of session metadata.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::StatusPageState;

/// Key-value rows for the Status tab.
struct StatusRow {
    label: &'static str,
    value: String,
    value_style: Style,
}

/// Render the Status tab. Returns the total row count for scroll clamping.
pub(super) fn render_status_tab(f: &mut Frame, state: &StatusPageState, area: Rect) -> usize {
    let s = &state.session;
    let c = &state.config;
    let mode_style = if s.mode == "plan" {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::Green).bold()
    };

    let rows = [
        row("Session ID", &s.session_id, default_style()),
        row("CWD", &s.cwd, Style::default().fg(Color::White)),
        row("Auth Token", &display_or_none(&c.auth_env), default_style()),
        row(
            "Base URL",
            &display_or_default(&c.base_url),
            default_style(),
        ),
        row("Model", &s.model_display, Style::default().fg(Color::Cyan)),
        row("Mode", &s.mode.to_uppercase(), mode_style),
        row(
            "Hub Endpoint",
            &display_or_none(&s.hub_endpoint),
            default_style(),
        ),
        row(
            "MCP Servers",
            &mcp_summary(c.mcp_configured, c.mcp_enabled),
            default_style(),
        ),
        row("Sources", &c.setting_sources.join(", "), default_style()),
    ];

    let scroll = state.active_scroll();
    let visible = area.height as usize;
    // Clamp: when all rows fit on screen, no scrolling needed.
    let scroll = scroll.min(rows.len().saturating_sub(visible));

    for (i, r) in rows.iter().skip(scroll).take(visible).enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let row_area = Rect::new(area.x, y, area.width, 1);
        let line = Line::from(vec![
            Span::styled(
                format!("  {:<16}", r.label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(&r.value, r.value_style),
        ]);
        f.render_widget(Paragraph::new(line), row_area);
    }
    rows.len()
}

fn row(label: &'static str, value: &str, style: Style) -> StatusRow {
    StatusRow {
        label,
        value: value.to_string(),
        value_style: style,
    }
}

fn default_style() -> Style {
    Style::default().fg(Color::White)
}

fn display_or_default(s: &str) -> String {
    if s.is_empty() {
        "(default)".to_string()
    } else {
        s.to_string()
    }
}

fn display_or_none(s: &str) -> String {
    if s.is_empty() {
        "(none)".to_string()
    } else {
        s.to_string()
    }
}

fn mcp_summary(configured: usize, enabled: usize) -> String {
    if configured == 0 {
        "none configured".to_string()
    } else {
        format!("{configured} configured, {enabled} enabled")
    }
}
