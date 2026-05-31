use loopal_error::MemoryGraphError;

use super::{RecallParams, RecallResult, ScoredNode};
use crate::graph::score::{
    fts_score, importance_bonus, recall_edge_weight, recall_recency_bonus,
    recall_reinforcement_bonus, recall_ttl_penalty, recall_type_weight,
};
use crate::store::MemoryGraph;
use crate::store::types::{MemoryEdge, MemoryKind};

const DEFAULT_DIRECT_HITS: usize = 8;

pub(super) fn empty_result() -> RecallResult {
    RecallResult {
        direct_hits: Vec::new(),
        neighbors: Vec::new(),
        co_occurring: Vec::new(),
        trail: Vec::new(),
    }
}

pub(super) fn cap_anchors(
    mut explicit: Vec<ScoredNode>,
    mut fts: Vec<ScoredNode>,
    max_results: usize,
) -> Vec<ScoredNode> {
    let cap = max_results.max(1);
    explicit.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    explicit.truncate(cap);
    let fts_budget = cap.saturating_sub(explicit.len());
    fts.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fts.truncate(fts_budget);
    explicit.extend(fts);
    explicit
}

pub(super) fn cap_neighbors(
    neighbors: &mut Vec<ScoredNode>,
    anchor_count: usize,
    max_results: usize,
) {
    const MIN_NEIGHBOR_BUDGET: usize = 5;
    neighbors.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let remaining = max_results.saturating_sub(anchor_count);
    let neighbor_budget = remaining.min(neighbors.len());
    let with_floor = if anchor_count >= max_results {
        0
    } else {
        neighbor_budget.max(MIN_NEIGHBOR_BUDGET.min(remaining))
    };
    neighbors.truncate(with_floor);
}

pub(super) async fn resolve_anchors(
    graph: &MemoryGraph,
    params: &RecallParams,
) -> Result<(Vec<ScoredNode>, Vec<ScoredNode>), MemoryGraphError> {
    let mut explicit: Vec<ScoredNode> = Vec::new();
    let mut fts: Vec<ScoredNode> = Vec::new();

    if !params.anchor_names.is_empty() {
        let nodes = graph.get_nodes(&params.anchor_names).await?;
        explicit.extend(
            nodes
                .into_iter()
                .filter(|n| n.kind != MemoryKind::Index)
                .map(|n| {
                    let stats = graph.recall_stats_snapshot(&n.id);
                    let score = recall_type_weight(n.kind)
                        + recall_recency_bonus(n.updated_at, stats.as_ref())
                        + recall_reinforcement_bonus(stats.as_ref())
                        + importance_bonus(stats.as_ref())
                        - recall_ttl_penalty(&n);
                    ScoredNode {
                        node: n,
                        score,
                        distance: 0,
                    }
                }),
        );
    }

    if let Some(q) = &params.query {
        let seen: std::collections::HashSet<String> =
            explicit.iter().map(|s| s.node.id.clone()).collect();
        let hits = graph.search(q, None, DEFAULT_DIRECT_HITS).await?;
        fts.extend(
            hits.into_iter()
                .filter(|h| !seen.contains(&h.node.id))
                .filter(|h| h.node.kind != MemoryKind::Index)
                .map(|h| {
                    let stats = graph.recall_stats_snapshot(&h.node.id);
                    let score = fts_score(h.rank)
                        + recall_type_weight(h.node.kind)
                        + recall_recency_bonus(h.node.updated_at, stats.as_ref())
                        + recall_reinforcement_bonus(stats.as_ref())
                        + importance_bonus(stats.as_ref())
                        - recall_ttl_penalty(&h.node);
                    ScoredNode {
                        node: h.node,
                        score,
                        distance: 0,
                    }
                }),
        );
    }

    Ok((explicit, fts))
}

pub(super) fn max_edge_weight_to_anchors(
    edges: &[MemoryEdge],
    anchor_set: &std::collections::HashSet<String>,
) -> std::collections::HashMap<String, f32> {
    let mut max_weight: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    for e in edges {
        let other = if anchor_set.contains(&e.src_id) && !anchor_set.contains(&e.dst_id) {
            Some(e.dst_id.clone())
        } else if anchor_set.contains(&e.dst_id) && !anchor_set.contains(&e.src_id) {
            Some(e.src_id.clone())
        } else {
            None
        };
        if let Some(other) = other {
            let w = recall_edge_weight(e.kind);
            let entry = max_weight.entry(other).or_insert(0.0);
            if w > *entry {
                *entry = w;
            }
        }
    }
    max_weight
}
