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
