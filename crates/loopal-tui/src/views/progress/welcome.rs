/// Welcome banner rendering: ASCII art logo + slogan + model/path info.
use ratatui::prelude::*;

use loopal_session::types::SessionMessage;

/// Render the welcome banner.
///
/// Content format: `"model\npath"` (two lines separated by newline).
pub fn render_welcome(lines: &mut Vec<Line<'static>>, msg: &SessionMessage) {
    let mut parts = msg.content.splitn(2, '\n');
    let model = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    // ASCII art "LOOPAL" — gradient from green to cyan
    let logo_lines: &[&str] = &[
        r"  _        ___    ___   ____     _     _     ",
        r" | |      / _ \  / _ \ |  _ \   / \   | |    ",
        r" | |     | | | || | | || |_) | / _ \  | |    ",
        r" | |___  | |_| || |_| ||  __/ / ___ \ | |___ ",
        r" |_____|  \___/  \___/ |_|   /_/   \_\|_____|",
    ];

    let gradient: &[Color] = &[
        Color::Rgb(80, 200, 120),
        Color::Rgb(70, 200, 150),
        Color::Rgb(60, 195, 180),
        Color::Rgb(50, 190, 210),
        Color::Rgb(40, 180, 230),
    ];

    lines.push(Line::from(""));
    for (i, logo_line) in logo_lines.iter().enumerate() {
        let color = gradient[i % gradient.len()];
        lines.push(Line::from(Span::styled(
            logo_line.to_string(),
            Style::default().fg(color).bold(),
        )));
    }

    // Slogan
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Rooted in code, Growing with ",
            Style::default().fg(Color::Rgb(140, 150, 170)),
        ),
        Span::styled(
            "loopal",
            Style::default().fg(Color::Rgb(60, 195, 180)).bold(),
        ),
        Span::styled(".", Style::default().fg(Color::Rgb(140, 150, 170))),
    ]));
    lines.push(Line::from(Span::styled(
        "  Part of AgentsMesh.ai",
        Style::default().fg(Color::Rgb(100, 110, 130)),
    )));

    // Info section
    lines.push(Line::from(""));
    if !model.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "  model:     ",
                Style::default()
                    .fg(Color::Rgb(100, 110, 130))
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(model.to_string(), Style::default().fg(Color::Cyan)),
        ]));
    }
    if !path.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "  directory: ",
                Style::default()
                    .fg(Color::Rgb(100, 110, 130))
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(
                path.to_string(),
                Style::default().fg(Color::Rgb(160, 170, 190)),
            ),
        ]));
    }
}
