/// Graphical agent topology overlay — Canvas-based network graph in top-right corner.
/// Shows parent/child relationships with animated status indicators.
mod layout;

use std::time::Duration;

use indexmap::IndexMap;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, canvas::Canvas};

use loopal_protocol::AgentStatus;
use loopal_session::state::AgentViewState;

use crate::views::unified_status::spinner_frame;
use layout::abbreviate_model;

/// Lightweight snapshot of one agent node for topology rendering.
#[derive(Clone)]
pub struct TopologyNode {
    pub name: String,
    pub status: AgentStatus,
    pub model: String,
    pub elapsed: Duration,
    pub tools_in_flight: u32,
    pub parent: Option<String>,
    pub children: Vec<String>,
}

/// Positioned node with canvas coordinates for rendering.
pub(crate) struct PlacedNode {
    pub node: TopologyNode,
    pub x: f64,
    pub y: f64,
}

/// Extract topology snapshot from SessionState. Includes a virtual root node.
///
/// Agents whose parent is absent or not a known sub-agent are treated as
/// direct children of the virtual "root" node. This handles the case where
/// the Hub registers the parent as "main" (the root agent process name).
pub fn extract_topology(
    agents: &IndexMap<String, AgentViewState>,
    root_model: &str,
    root_status: AgentStatus,
    root_elapsed: Duration,
) -> Vec<TopologyNode> {
    // Filter out finished/errored agents — they are no longer active.
    let live_agents: IndexMap<&String, &AgentViewState> = agents
        .iter()
        .filter(|(_, a)| {
            !matches!(
                a.observable.status,
                AgentStatus::Finished | AgentStatus::Error
            )
        })
        .collect();

    // An agent is a direct child of root if its parent is None or not in the agents map.
    let child_names: Vec<String> = live_agents
        .iter()
        .filter(|(_, a)| match &a.parent {
            None => true,
            Some(p) => !live_agents.contains_key(p),
        })
        .map(|(name, _)| (*name).clone())
        .collect();

    let mut nodes = vec![TopologyNode {
        name: "root".into(),
        status: root_status,
        model: abbreviate_model(root_model),
        elapsed: root_elapsed,
        tools_in_flight: 0,
        parent: None,
        children: child_names,
    }];

    for (name, agent) in &live_agents {
        // Remap parent to "root" if the parent is not a known live sub-agent.
        let parent = match &agent.parent {
            Some(p) if live_agents.contains_key(p) => Some(p.clone()),
            _ => Some("root".into()),
        };
        nodes.push(TopologyNode {
            name: (*name).clone(),
            status: agent.observable.status,
            model: abbreviate_model(&agent.observable.model),
            elapsed: agent.elapsed(),
            tools_in_flight: agent.observable.tools_in_flight,
            parent,
            children: agent
                .children
                .iter()
                .filter(|c| live_agents.contains_key(c))
                .cloned()
                .collect(),
        });
    }
    nodes
}

/// Render the topology overlay in the top-right corner.
pub fn render_topology_overlay(f: &mut Frame, nodes: &[TopologyNode], area: Rect) {
    if nodes.len() <= 1 {
        return; // Only root, no sub-agents
    }

    let placed = layout::compute_layout(nodes);
    if placed.is_empty() {
        return;
    }

    // Compute overlay dimensions based on tree shape
    let overlay_w = layout::compute_overlay_width(&placed, area.width);
    let overlay_h = layout::compute_overlay_height(&placed, area.height);
    let x = area.x + area.width.saturating_sub(overlay_w + 1);
    let y = area.y + 1;
    let overlay_area = Rect::new(x, y, overlay_w, overlay_h);

    f.render_widget(Clear, overlay_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Topology ")
        .border_style(Style::default().fg(Color::Rgb(100, 100, 140)))
        .style(Style::default().bg(Color::Rgb(20, 20, 28)));

    let inner = block.inner(overlay_area);
    f.render_widget(block, overlay_area);

    // Render with Canvas inside the inner area
    render_graph(f, &placed, inner);
}

fn render_graph(f: &mut Frame, placed: &[PlacedNode], area: Rect) {
    if area.width < 4 || area.height < 2 {
        return;
    }

    // Canvas coordinate bounds (with padding)
    let (x_min, x_max, y_min, y_max) = layout::canvas_bounds(placed);

    let canvas = Canvas::default()
        .x_bounds([x_min - 2.0, x_max + 2.0])
        .y_bounds([y_min - 1.0, y_max + 1.0])
        .paint(|ctx| {
            // Draw edges first (behind nodes)
            for node in placed {
                for child in placed {
                    if child.node.parent.as_deref() == Some(&node.node.name) {
                        ctx.draw(&ratatui::widgets::canvas::Line {
                            x1: node.x,
                            y1: node.y,
                            x2: child.x,
                            y2: child.y,
                            color: Color::Rgb(60, 60, 80),
                        });
                    }
                }
            }

            // Draw node labels
            for pn in placed {
                let (icon, color) = status_icon(&pn.node);
                let label = format!("{icon} {} ({})", pn.node.name, pn.node.model);
                ctx.print(pn.x, pn.y, label.fg(color));
            }
        });

    f.render_widget(canvas, area);
}

fn status_icon(node: &TopologyNode) -> (&'static str, Color) {
    match node.status {
        AgentStatus::Starting => (spinner_frame(node.elapsed), Color::DarkGray),
        AgentStatus::Running => (spinner_frame(node.elapsed), Color::Green),
        AgentStatus::WaitingForInput => ("●", Color::DarkGray),
        AgentStatus::Finished => ("✓", Color::Green),
        AgentStatus::Error => ("✗", Color::Red),
    }
}
