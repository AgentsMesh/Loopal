//! Status page sub-page — tabbed dashboard with Status / Config / Usage.

mod config_tab;
mod status_tab;
mod usage_tab;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear};

use crate::app::{StatusPageState, StatusTab};

/// Render the full-screen status page sub-page.
///
/// Takes `&mut` to write back the clamped scroll offset after rendering,
/// preventing scroll accumulation beyond the visible content.
pub fn render_status_page(f: &mut Frame, state: &mut StatusPageState, area: Rect) {
    f.render_widget(Clear, area);

    let tab_bar = build_tab_bar(state.active_tab);
    let hint_bar = build_hint_bar(state.active_tab);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(tab_bar)
        .title_bottom(hint_bar)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        let row_count = state.active_row_count();
        clamp_scroll(state, inner.height, row_count);
        return;
    }

    // Each tab renderer returns its total row count so clamp_scroll
    // does not need to recompute filtered_config().
    let row_count = match state.active_tab {
        StatusTab::Status => status_tab::render_status_tab(f, state, inner),
        StatusTab::Config => config_tab::render_config_tab(f, state, inner),
        StatusTab::Usage => usage_tab::render_usage_tab(f, state, inner),
    };

    // Write back clamped scroll so the key handler never accumulates
    // beyond what the render can actually display.
    clamp_scroll(state, inner.height, row_count);
}

/// Clamp the active tab's scroll offset to the renderable range.
fn clamp_scroll(state: &mut StatusPageState, inner_height: u16, row_count: usize) {
    let content_height = match state.active_tab {
        StatusTab::Config => (inner_height.saturating_sub(2)) as usize, // header + separator
        _ => inner_height as usize,
    };
    let max_scroll = row_count.saturating_sub(content_height);
    let scroll = state.active_scroll_mut();
    if *scroll > max_scroll {
        *scroll = max_scroll;
    }
}

/// Build the tab bar title with active tab highlighted.
fn build_tab_bar(active: StatusTab) -> Line<'static> {
    let mut spans = Vec::with_capacity(8);
    spans.push(Span::raw(" "));

    for (i, tab) in StatusTab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", Style::default().fg(Color::DarkGray)));
        }
        if *tab == active {
            spans.push(Span::styled(
                tab.label(),
                Style::default().fg(Color::Cyan).bold(),
            ));
        } else {
            spans.push(Span::styled(
                tab.label(),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Build the bottom hint bar.
fn build_hint_bar(active: StatusTab) -> Line<'static> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled("\u{2190}/\u{2192}", Style::default().fg(Color::Cyan)),
        Span::raw(" tab  "),
        Span::styled("\u{2191}/\u{2193}", Style::default().fg(Color::Cyan)),
        Span::raw(" scroll  "),
    ];
    if active == StatusTab::Config {
        spans.push(Span::styled("type", Style::default().fg(Color::Green)));
        spans.push(Span::raw(" filter  "));
    }
    spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
    spans.push(Span::raw(" close "));
    Line::from(spans)
}
