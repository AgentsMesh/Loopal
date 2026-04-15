//! MCP server status page — renders the list of MCP servers and their state.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::McpPageState;

pub fn render_mcp_page(f: &mut Frame, state: &mut McpPageState, area: Rect) {
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("MCP Servers", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" "),
        ]))
        .title_bottom(build_hint_bar())
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    if !state.loaded {
        let msg = Paragraph::new("  Loading MCP server status...")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, inner);
        return;
    }

    if state.servers.is_empty() {
        let msg = Paragraph::new("  No MCP servers configured")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, inner);
        return;
    }

    let detail_width = 40.min(inner.width / 2);
    let list_width = inner.width.saturating_sub(detail_width);
    let list_area = Rect::new(inner.x, inner.y, list_width, inner.height);
    let detail_area = Rect::new(inner.x + list_width, inner.y, detail_width, inner.height);

    render_server_list(f, state, list_area);
    render_server_detail(f, state, detail_area);
}

fn render_server_list(f: &mut Frame, state: &mut McpPageState, area: Rect) {
    let visible = area.height as usize;
    let max_scroll = state.servers.len().saturating_sub(visible);
    if state.scroll_offset > max_scroll {
        state.scroll_offset = max_scroll;
    }

    let scroll = state.scroll_offset;
    for (i, server) in state.servers.iter().skip(scroll).take(visible).enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let idx = scroll + i;
        let is_selected = idx == state.selected;
        let status_icon = status_indicator(&server.status);
        let prefix = if is_selected { "\u{25b8} " } else { "  " };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(status_icon, status_style(&server.status)),
            Span::raw(" "),
            Span::styled(
                &server.name,
                if is_selected {
                    Style::default().fg(Color::White).bold()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::styled(
                format!("  [{}]", server.transport),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        f.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
    }
}

fn render_server_detail(f: &mut Frame, state: &McpPageState, area: Rect) {
    let Some(server) = state.selected_server() else {
        return;
    };
    if area.height < 4 || area.width < 10 {
        return;
    }

    let tool_str = server.tool_count.to_string();
    let res_str = server.resource_count.to_string();
    let prompt_str = server.prompt_count.to_string();
    let mut lines: Vec<Line> = vec![
        detail_row("Name", &server.name, Color::White),
        detail_row("Transport", &server.transport, Color::White),
        detail_row("Source", &server.source, Color::White),
        detail_row("Status", &server.status, status_color(&server.status)),
        detail_row("Tools", &tool_str, Color::Cyan),
        detail_row("Resources", &res_str, Color::Cyan),
        detail_row("Prompts", &prompt_str, Color::Cyan),
    ];

    if !server.errors.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            " Errors:",
            Style::default().fg(Color::Red).bold(),
        )));
        for err in &server.errors {
            let max_len = (area.width as usize).saturating_sub(4);
            let truncated = if err.len() > max_len {
                let end = err
                    .char_indices()
                    .take_while(|(i, _)| *i < max_len.saturating_sub(1))
                    .last()
                    .map_or(0, |(i, c)| i + c.len_utf8());
                format!(" {}\u{2026}", &err[..end])
            } else {
                format!(" {err}")
            };
            lines.push(Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::Red),
            )));
        }
    }

    for (i, line) in lines.iter().take(area.height as usize).enumerate() {
        let y = area.y + i as u16;
        f.render_widget(
            Paragraph::new(line.clone()),
            Rect::new(area.x, y, area.width, 1),
        );
    }
}

fn detail_row<'a>(label: &'a str, value: &'a str, value_color: Color) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!(" {label:<12}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value, Style::default().fg(value_color)),
    ])
}

fn status_indicator(status: &str) -> &'static str {
    if status == "connected" {
        "\u{25cf}" // ● filled = connected
    } else if status.starts_with("failed") {
        "\u{2717}" // ✗ cross = failed
    } else {
        "\u{25cb}" // ○ hollow = disconnected/connecting
    }
}

fn status_style(status: &str) -> Style {
    Style::default().fg(status_color(status))
}

fn status_color(status: &str) -> Color {
    if status == "connected" {
        Color::Green
    } else if status.starts_with("failed") {
        Color::Red
    } else if status == "connecting" {
        Color::Yellow
    } else {
        Color::DarkGray
    }
}

fn build_hint_bar() -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{2191}/\u{2193}", Style::default().fg(Color::Cyan)),
        Span::raw(" navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::raw(" reconnect  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" close "),
    ])
}
