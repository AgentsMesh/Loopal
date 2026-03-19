/// Agent status panel — displays per-agent observability data.
///
/// Shows running agents with status, tool count, turns, token usage, and focus
/// indicator. Collapses to 0 height when no agents are active.
use indexmap::IndexMap;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::AgentViewState;
use loopal_types::agent_state::AgentStatus;

/// Panel height: 1 if any agent is active, 0 otherwise.
pub fn panel_height(agents: &IndexMap<String, AgentViewState>) -> u16 {
    let has_active = agents.values().any(|a| {
        matches!(a.observable.status, AgentStatus::Running | AgentStatus::Starting)
    });
    if has_active { 1 } else { 0 }
}

/// Render agent panel as a compact single-line display.
///
/// Format: ` [~] name (N tools, T turns, Xk tok) [*focused]`
pub fn render_subagent_panel(
    f: &mut Frame,
    agents: &IndexMap<String, AgentViewState>,
    focused_agent: Option<&str>,
    area: Rect,
) {
    if area.height == 0 || agents.is_empty() {
        return;
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (name, av) in agents {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        let (icon, style) = status_style(&av.observable.status);
        let tok_k = (av.observable.input_tokens + av.observable.output_tokens) / 1000;
        let info = format!(
            " [{icon}] {name} ({} tools, {} turns, {tok_k}k tok)",
            av.observable.tool_count, av.observable.turn_count,
        );
        let is_focused = focused_agent == Some(name.as_str());
        let final_style = if is_focused {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };
        spans.push(Span::styled(info, final_style));
        if is_focused {
            spans.push(Span::styled(" *", Style::default().fg(Color::Magenta)));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Map AgentStatus to (icon, style) pair.
fn status_style(status: &AgentStatus) -> (&'static str, Style) {
    match status {
        AgentStatus::Starting => (".", Style::default().fg(Color::DarkGray)),
        AgentStatus::Running => ("~", Style::default().fg(Color::Yellow)),
        AgentStatus::WaitingForInput => ("?", Style::default().fg(Color::Cyan)),
        AgentStatus::Finished => ("+", Style::default().fg(Color::Green)),
        AgentStatus::Error => ("x", Style::default().fg(Color::Red)),
    }
}
