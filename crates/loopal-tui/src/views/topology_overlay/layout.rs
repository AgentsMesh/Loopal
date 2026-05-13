/// Tree layout algorithm — positions topology nodes on a canvas grid.
///
/// BFS traversal places root at top-center, each level's children spread
/// horizontally below their parent. Coordinates use canvas units (not cells).
use std::collections::VecDeque;

use super::{PlacedNode, TopologyNode};

/// Horizontal spacing between sibling nodes (canvas units).
const H_SPACING: f64 = 16.0;

/// Vertical spacing between tree levels (canvas units).
const V_SPACING: f64 = 4.0;

/// Compute (x, y) positions for all nodes using a layered tree layout.
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

    // Y-axis: root at top (highest Y value), children below
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

/// Assign nodes to BFS levels starting from root(s).
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
        // Ensure level vector exists
        while levels.len() <= depth {
            levels.push(Vec::new());
        }

        // Enqueue children
        for child_name in &node.children {
            if let Some(child) = nodes.iter().find(|n| &n.name == child_name) {
                queue.push_back((child.clone(), depth + 1));
            }
        }

        levels[depth].push(node);
    }

    levels
}

/// Abbreviate model name for compact display.
/// "claude-sonnet-4-20250514" → "sonnet-4", "claude-opus-4-6" → "opus-4"
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

/// Width adapts to the widest tree level (max label + spacing per node).
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

// reason: u16::clamp panics if lower > upper; in TUI render paths the upper
// bound is derived from a runtime area size that can drop below the lower
// constant on tiny terminals. Collapse the lower bound onto the upper so the
// invariant holds, then expand the upper to never sit below the lower.
fn safe_clamp(value: u16, lower: u16, upper: u16) -> u16 {
    let lo = lower.min(upper);
    let hi = upper.max(lo);
    value.clamp(lo, hi)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use loopal_protocol::AgentStatus;

    use super::*;

    fn make_node(name: &str, parent: Option<&str>, children: &[&str]) -> TopologyNode {
        TopologyNode {
            name: name.into(),
            status: AgentStatus::Running,
            model: "test".into(),
            elapsed: Duration::ZERO,
            tools_in_flight: 0,
            parent: parent.map(String::from),
            children: children.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn single_root_places_at_origin() {
        let nodes = vec![make_node("root", None, &[])];
        let placed = compute_layout(&nodes);
        assert_eq!(placed.len(), 1);
        assert!((placed[0].x).abs() < 0.01);
    }

    #[test]
    fn two_level_tree_has_correct_depth() {
        let nodes = vec![
            make_node("root", None, &["a", "b"]),
            make_node("a", Some("root"), &[]),
            make_node("b", Some("root"), &[]),
        ];
        let placed = compute_layout(&nodes);
        assert_eq!(placed.len(), 3);
        // Root at top (higher Y), children at lower Y
        let root_y = placed.iter().find(|p| p.node.name == "root").unwrap().y;
        let child_y = placed.iter().find(|p| p.node.name == "a").unwrap().y;
        assert!(root_y > child_y);
    }

    #[test]
    fn children_spread_horizontally() {
        let nodes = vec![
            make_node("root", None, &["a", "b", "c"]),
            make_node("a", Some("root"), &[]),
            make_node("b", Some("root"), &[]),
            make_node("c", Some("root"), &[]),
        ];
        let placed = compute_layout(&nodes);
        let xs: Vec<f64> = placed
            .iter()
            .filter(|p| p.node.parent.is_some())
            .map(|p| p.x)
            .collect();
        // Check they are evenly spaced
        assert_eq!(xs.len(), 3);
        assert!((xs[1] - xs[0] - H_SPACING).abs() < 0.01);
    }

    #[test]
    fn overlay_width_does_not_panic_on_tiny_terminal() {
        let placed = compute_layout(&[
            make_node("root", None, &["a"]),
            make_node("a", Some("root"), &[]),
        ]);
        for w in [0u16, 1, 10, 20, 39, 40, 80, 200] {
            let _ = compute_overlay_width(&placed, w);
        }
    }

    #[test]
    fn overlay_height_does_not_panic_on_tiny_terminal() {
        let placed = compute_layout(&[
            make_node("root", None, &["a"]),
            make_node("a", Some("root"), &[]),
        ]);
        for h in [0u16, 1, 7, 14, 15, 16, 24, 100] {
            let _ = compute_overlay_height(&placed, h);
        }
    }

    #[test]
    fn safe_clamp_handles_inverted_bounds() {
        assert_eq!(safe_clamp(10, 8, 4), 4);
        assert_eq!(safe_clamp(2, 8, 4), 4);
        assert_eq!(safe_clamp(100, 8, 0), 0);
        assert_eq!(safe_clamp(50, 10, 80), 50);
        assert_eq!(safe_clamp(5, 10, 80), 10);
        assert_eq!(safe_clamp(200, 10, 80), 80);
    }
}
