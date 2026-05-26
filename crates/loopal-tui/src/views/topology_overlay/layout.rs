use std::collections::VecDeque;

use super::{PlacedNode, TopologyNode};

const H_SPACING: f64 = 16.0;
const V_SPACING: f64 = 4.0;

pub(super) fn compute_layout(nodes: &[TopologyNode]) -> Vec<PlacedNode> {
    if nodes.is_empty() {
        return Vec::new();
    }

    let levels = assign_levels(nodes);
    let max_depth = levels.iter().map(|l| l.len()).max().unwrap_or(0);
    if max_depth == 0 {
        return Vec::new();
    }

    let mut placed = Vec::with_capacity(nodes.len());

    let max_y = (levels.len() as f64 - 1.0) * V_SPACING;

    for (depth, level) in levels.iter().enumerate() {
        let y = max_y - (depth as f64 * V_SPACING);
        let count = level.len() as f64;
        let total_width = (count - 1.0) * H_SPACING;
        let start_x = -total_width / 2.0;

        for (i, node) in level.iter().enumerate() {
            placed.push(PlacedNode {
                node: node.clone(),
                x: start_x + i as f64 * H_SPACING,
                y,
            });
        }
    }

    placed
}

fn assign_levels(nodes: &[TopologyNode]) -> Vec<Vec<TopologyNode>> {
    let roots: Vec<&TopologyNode> = nodes.iter().filter(|n| n.parent.is_none()).collect();
    if roots.is_empty() {
        return vec![nodes.to_vec()];
    }

    let mut levels: Vec<Vec<TopologyNode>> = Vec::new();
    let mut queue: VecDeque<(TopologyNode, usize)> = VecDeque::new();

    for root in roots {
        queue.push_back((root.clone(), 0));
    }

    while let Some((node, depth)) = queue.pop_front() {
        while levels.len() <= depth {
            levels.push(Vec::new());
        }

        for child_name in &node.children {
            if let Some(child) = nodes.iter().find(|n| &n.name == child_name) {
                queue.push_back((child.clone(), depth + 1));
            }
        }

        levels[depth].push(node);
    }

    levels
}

// "claude-sonnet-4-20250514" → "sonnet-4", "claude-opus-4-6" → "opus-4"
pub fn abbreviate_model(model: &str) -> String {
    let parts: Vec<&str> = model.split('-').collect();
    if parts.len() >= 3 && parts[0] == "claude" {
        let name = parts[1];
        let ver = parts.get(2).unwrap_or(&"");
        format!("{name}-{ver}")
    } else if model.len() > 10 {
        model.chars().take(10).collect()
    } else {
        model.to_string()
    }
}

pub fn canvas_bounds(placed: &[PlacedNode]) -> (f64, f64, f64, f64) {
    placed.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(xn, xx, yn, yx), p| (xn.min(p.x), xx.max(p.x), yn.min(p.y), yx.max(p.y)),
    )
}

pub fn compute_overlay_width(placed: &[PlacedNode], max_w: u16) -> u16 {
    let (x_min, x_max, _, _) = canvas_bounds(placed);
    let max_label = placed
        .iter()
        .map(|p| p.node.name.len() + p.node.model.len() + 6)
        .max()
        .unwrap_or(12);
    let span = ((x_max - x_min) / 4.0).ceil() as u16 + 1;
    let desired = span.max(max_label as u16) + 4;
    let upper = max_w.saturating_mul(60) / 100;
    safe_clamp(desired, 24, upper)
}

pub fn compute_overlay_height(placed: &[PlacedNode], max_h: u16) -> u16 {
    let (_, _, y_min, y_max) = canvas_bounds(placed);
    let depth = ((y_max - y_min) / 4.0).ceil() as u16 + 1;
    let desired = depth.saturating_mul(3) + 2;
    safe_clamp(desired, 8, max_h / 2)
}

// u16::clamp panics when lower > upper; render-path upper bound is derived
// from runtime area size and can drop below the lower constant on tiny
// terminals. Collapse to the smaller value first to preserve the invariant.
fn safe_clamp(value: u16, lower: u16, upper: u16) -> u16 {
    let lo = lower.min(upper);
    let hi = upper.max(lo);
    value.clamp(lo, hi)
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
