//! Skills sub-page — renders the list of loaded skills and their details.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::SkillsPageState;

pub fn render_skills_page(f: &mut Frame, state: &mut SkillsPageState, area: Rect) {
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("Skills", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" "),
        ]))
        .title_bottom(build_hint_bar())
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    if state.skills.is_empty() {
        let msg = Paragraph::new("  No skills loaded")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, inner);
        return;
    }

    let detail_width = 40.min(inner.width / 2);
    let list_width = inner.width.saturating_sub(detail_width);
    let list_area = Rect::new(inner.x, inner.y, list_width, inner.height);
    let detail_area = Rect::new(inner.x + list_width, inner.y, detail_width, inner.height);

    render_skill_list(f, state, list_area);
    render_skill_detail(f, state, detail_area);
}

fn render_skill_list(f: &mut Frame, state: &mut SkillsPageState, area: Rect) {
    let visible = area.height as usize;
    if state.selected < state.scroll_offset {
        state.scroll_offset = state.selected;
    } else if visible > 0 && state.selected >= state.scroll_offset + visible {
        state.scroll_offset = state.selected - visible + 1;
    }
    let max_scroll = state.skills.len().saturating_sub(visible);
    if state.scroll_offset > max_scroll {
        state.scroll_offset = max_scroll;
    }

    let scroll = state.scroll_offset;
    for (i, skill) in state.skills.iter().skip(scroll).take(visible).enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let idx = scroll + i;
        let is_selected = idx == state.selected;
        let prefix = if is_selected { "\u{25b8} " } else { "  " };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(
                &skill.name,
                if is_selected {
                    Style::default().fg(Color::White).bold()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::styled(
                format!("  [{}]", skill.source),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        f.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
    }
}

fn render_skill_detail(f: &mut Frame, state: &SkillsPageState, area: Rect) {
    let Some(skill) = state.selected_skill() else {
        return;
    };
    if area.height < 4 || area.width < 10 {
        return;
    }

    let arg_label = if skill.has_arg { "yes" } else { "no" };
    let lines: Vec<Line> = vec![
        detail_row("Name", &skill.name, Color::White),
        detail_row("Source", &skill.source, Color::White),
        detail_row("Args", arg_label, Color::Cyan),
        Line::raw(""),
        Line::from(Span::styled(
            " Description:",
            Style::default().fg(Color::DarkGray).bold(),
        )),
        Line::from(Span::styled(
            format!(" {}", &skill.description),
            Style::default().fg(Color::White),
        )),
    ];

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

fn build_hint_bar() -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{2191}/\u{2193}", Style::default().fg(Color::Cyan)),
        Span::raw(" navigate  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" close "),
    ])
}
