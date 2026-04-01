/// Agent status panel — per-agent observability with focus selection.
///
/// Dynamic height: 0 when no live agents, 1 line per agent otherwise.
/// Tab cycles focus; focused agent highlighted with `▸`.
///
/// ```text
///  ▸ researcher   ⠹ Working   12s  Read(src/foo.rs)
///    coder        ● Idle        0s
///    tester       ⠧ Working     5s  Bash(npm test)
/// ```
use indexmap::IndexMap;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_protocol::AgentStatus;
use loopal_session::state::AgentViewState;

use super::unified_status::{format_duration, spinner_frame};

/// Maximum visible agent rows before showing overflow.
/// NOTE: must match `key_dispatch_ops::MAX_VISIBLE` (scroll calculation).
pub const MAX_VISIBLE: usize = 5;

/// Compute the height needed for the agent panel.
/// Excludes the currently viewed agent — it's the active conversation, not a switchable target.
pub fn panel_height(
    agents: &IndexMap<String, AgentViewState>,
    active_view: &str,
    agent_panel_offset: usize,
) -> u16 {
    let live = agents
        .iter()
        .filter(|(name, a)| name.as_str() != active_view && is_live(&a.observable.status))
        .count();
    if live == 0 {
        return 0;
    }
    let visible = live.min(MAX_VISIBLE);
    let clamped = agent_panel_offset.min(live.saturating_sub(MAX_VISIBLE));
    let has_above = clamped > 0;
    let has_below = live > clamped + MAX_VISIBLE;
    let indicators = u16::from(has_above) + u16::from(has_below);
    visible as u16 + indicators
}

/// Render the agent panel with a scrolling viewport.
/// `active_view` is excluded from the list (it's the current conversation).
pub fn render_agent_panel(
    f: &mut Frame,
    agents: &IndexMap<String, AgentViewState>,
    focused: Option<&str>,
    viewing: Option<&str>,
    active_view: &str,
    agent_panel_offset: usize,
    area: Rect,
) {
    if area.height == 0 || agents.is_empty() {
        return;
    }

    let max_name = agents.keys().map(|n| n.len()).max().unwrap_or(0).max(4);
    let live_agents: Vec<_> = agents
        .iter()
        .filter(|(name, a)| name.as_str() != active_view && is_live(&a.observable.status))
        .collect();

    let total = live_agents.len();
    let offset = agent_panel_offset.min(total.saturating_sub(MAX_VISIBLE));
    let window_end = (offset + MAX_VISIBLE).min(total);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if offset > 0 {
        lines.push(Line::from(Span::styled(
            format!("  \u{2191} {offset} more"),
            Style::default().fg(Color::DarkGray),
        )));
    }

    for (name, agent) in &live_agents[offset..window_end] {
        let is_focused = focused == Some(name.as_str());
        let is_viewing = viewing == Some(name.as_str());
        lines.push(render_agent_line(
            name, agent, is_focused, is_viewing, max_name,
        ));
    }

    if window_end < total {
        lines.push(Line::from(Span::styled(
            format!("  \u{2193} {} more", total - window_end),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let bg = Style::default().bg(Color::Rgb(25, 25, 30));
    f.render_widget(Paragraph::new(lines).style(bg), area);
}

/// Render a single agent status line.
fn render_agent_line(
    name: &str,
    agent: &AgentViewState,
    is_focused: bool,
    is_viewing: bool,
    name_width: usize,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(10);

    // Focus / viewing indicator
    let (indicator, base_style) = if is_viewing {
        (" ▶ ", Style::default().fg(Color::Green).bold())
    } else if is_focused {
        (" ▸ ", Style::default().fg(Color::Cyan).bold())
    } else {
        ("   ", Style::default())
    };
    spans.push(Span::styled(indicator.to_string(), base_style));

    // Name (padded for column alignment)
    let padded = format!("{name:<name_width$}");
    let name_style = if is_viewing {
        Style::default().fg(Color::Green).bold()
    } else if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::White)
    };
    spans.push(Span::styled(padded, name_style));
    spans.push(Span::raw("  "));

    // Spinner/icon + status label
    let elapsed = agent.elapsed();
    let (icon, label, icon_style) = status_display(
        &agent.observable.status,
        agent.observable.tools_in_flight,
        elapsed,
    );
    spans.push(Span::styled(format!("{icon} {label:<12}"), icon_style));
    spans.push(Span::raw(" "));

    // Elapsed time
    let time_str = if elapsed.as_secs() > 0 {
        format_duration(elapsed)
    } else {
        "-".to_string()
    };
    spans.push(Span::styled(
        format!("{time_str:>6}"),
        Style::default().fg(Color::DarkGray),
    ));

    // Last tool (truncated)
    if let Some(ref tool) = agent.observable.last_tool {
        spans.push(Span::raw("  "));
        let display: String = tool.chars().take(20).collect();
        spans.push(Span::styled(
            display,
            Style::default().fg(Color::Rgb(80, 80, 80)),
        ));
    }

    Line::from(spans)
}

/// Map agent status to (icon, label, style) for display.
fn status_display(
    status: &AgentStatus,
    tools_in_flight: u32,
    elapsed: std::time::Duration,
) -> (&'static str, String, Style) {
    match status {
        AgentStatus::Starting => {
            let frame = spinner_frame(elapsed);
            (
                frame,
                "Starting".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        }
        AgentStatus::Running => {
            let frame = spinner_frame(elapsed);
            let label = if tools_in_flight > 1 {
                format!("Working ({tools_in_flight})")
            } else if tools_in_flight == 0 {
                "Thinking".to_string()
            } else {
                "Working".to_string()
            };
            (frame, label, Style::default().fg(Color::Green))
        }
        AgentStatus::WaitingForInput => (
            "●",
            "Idle".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
        AgentStatus::Finished => ("✓", "Done".to_string(), Style::default().fg(Color::Green)),
        AgentStatus::Error => ("✗", "Error".to_string(), Style::default().fg(Color::Red)),
    }
}

fn is_live(status: &AgentStatus) -> bool {
    !matches!(status, AgentStatus::Finished | AgentStatus::Error)
}
