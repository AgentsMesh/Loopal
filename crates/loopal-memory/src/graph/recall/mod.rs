use loopal_error::MemoryGraphError;

use crate::graph::co_occur;
use crate::graph::score::{
    importance_bonus, neighbor_decay, recall_recency_bonus, recall_reinforcement_bonus,
    recall_ttl_penalty,
};
use crate::store::types::{MemoryEdge, MemoryKind, MemoryNode, Provenance};
use crate::store::{BfsConfig, MemoryGraph};

mod anchors;
mod emit;

use anchors::{
    cap_anchors, cap_neighbors, empty_result, max_edge_weight_to_anchors, resolve_anchors,
};
use emit::emit_events;

const DEFAULT_MAX_RESULTS: usize = 12;
const DEFAULT_DEPTH: u32 = 2;
const DEFAULT_MIN_CONFIDENCE: f32 = 0.3;
const ANCHOR_EDGE_BONUS_SCALE: f32 = 0.4;

pub struct RecallParams {
    pub query: Option<String>,
    pub anchor_names: Vec<String>,
    pub max_results: usize,
    pub depth: u32,
    pub include_synthesized: bool,
    pub min_confidence: f32,
}

impl Default for RecallParams {
    fn default() -> Self {
        Self {
            query: None,
            anchor_names: Vec::new(),
            max_results: DEFAULT_MAX_RESULTS,
            depth: DEFAULT_DEPTH,
            include_synthesized: true,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
        }
    }
}

pub struct ScoredNode {
    pub node: MemoryNode,
    pub score: f32,
    pub distance: u32,
}

pub struct CoOccurringNode {
    pub node: MemoryNode,
    pub weight: f32,
    pub via_anchor: String,
}

pub struct RecallResult {
    pub direct_hits: Vec<ScoredNode>,
    pub neighbors: Vec<ScoredNode>,
    pub co_occurring: Vec<CoOccurringNode>,
    pub trail: Vec<MemoryEdge>,
}

pub async fn recall(
    graph: &MemoryGraph,
    params: &RecallParams,
) -> Result<RecallResult, MemoryGraphError> {
    let (explicit, fts) = resolve_anchors(graph, params).await?;
    if explicit.is_empty() && fts.is_empty() {
        return Ok(empty_result());
    }
    let anchors = cap_anchors(explicit, fts, params.max_results);

    let anchor_ids: Vec<String> = anchors.iter().map(|s| s.node.id.clone()).collect();
    let exclude_provenance = if params.include_synthesized {
        Vec::new()
    } else {
        vec![Provenance::Synthesized]
    };
    // anchor-mode 显式指定种子时，放宽 BFS 阈值到 0.1，让大簇 (N≥5) slug_cluster
    // 自然衰减边 (1/(N-1)) 仍可被遍历到，保证 q06 类查询 R@10=1.0。
    // query-mode 沿用 params.min_confidence (默认 0.3)，让大簇噪声被剪掉。
    let bfs_min_confidence = if params.anchor_names.is_empty() {
        params.min_confidence
    } else {
        params.min_confidence.min(0.1)
    };
    let sub = graph
        .bfs(
            &anchor_ids,
            BfsConfig {
                max_depth: params.depth,
                edge_kinds: None,
                exclude_provenance,
                min_confidence: bfs_min_confidence,
                limit_nodes: params.max_results.max(20) * 4,
            },
        )
        .await?;

    let anchor_set: std::collections::HashSet<String> = anchor_ids.iter().cloned().collect();
    // 仅基于用户显式 anchor 计算 edge bonus：anchor 显式 wikilink/related
    // 邻居是强关系，应优先于 FTS hit 的邻居（避免 FTS 把不相干的 anchor 引入再拖来一堆噪声）。
    let explicit_anchor_set: std::collections::HashSet<String> =
        params.anchor_names.iter().cloned().collect();
    let edge_weight_to_anchor = max_edge_weight_to_anchors(&sub.edges, &explicit_anchor_set);
    let mut neighbors: Vec<ScoredNode> = sub
        .nodes
        .iter()
        .filter(|n| !anchor_set.contains(&n.id))
        .filter(|n| n.kind != MemoryKind::Index)
        .map(|n| {
            let distance = *sub.depth_by_id.get(&n.id).unwrap_or(&u32::MAX);
            let anchor_edge_bonus =
                edge_weight_to_anchor.get(&n.id).copied().unwrap_or(0.0) * ANCHOR_EDGE_BONUS_SCALE;
            let stats = graph.recall_stats_snapshot(&n.id);
            let score = neighbor_decay(distance)
                + anchor_edge_bonus
                + recall_recency_bonus(n.updated_at, stats.as_ref())
                + recall_reinforcement_bonus(stats.as_ref())
                + importance_bonus(stats.as_ref())
                - recall_ttl_penalty(n);
            ScoredNode {
                node: n.clone(),
                score,
                distance,
            }
        })
        .collect();
    cap_neighbors(&mut neighbors, anchors.len(), params.max_results);

    let co_occurring = co_occur::collect(graph, &anchor_ids, params).await?;

    let result = RecallResult {
        direct_hits: anchors,
        neighbors,
        co_occurring,
        trail: sub.edges,
    };
    emit_events(graph, params, &result);
    Ok(result)
}
