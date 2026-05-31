use chrono::Utc;

use crate::policy::{
    RANKING_ORPHAN_PENALTY, RANKING_RECENCY_DECAY_DAYS, RANKING_RECENCY_SCALE,
    RANKING_TTL_EXPIRED_PENALTY, RANKING_TTL_NEAR_PENALTY, TTL_NEAR_THRESHOLD_DAYS,
    ranking_kind_weight,
};
use crate::store::types::MemoryNode;

pub struct RankedNode {
    pub node: MemoryNode,
    pub score: f32,
    pub incoming_count: usize,
}

pub fn rank(node: MemoryNode, incoming_count: usize) -> RankedNode {
    let now = Utc::now().timestamp_millis();
    let incoming_score = (1.0 + incoming_count as f32).ln();
    let recency = recency_factor(node.updated_at, now);
    let type_w = ranking_kind_weight(node.kind);
    let ttl_d = ttl_decay(&node, now);
    let orphan_pen = if incoming_count == 0 {
        RANKING_ORPHAN_PENALTY
    } else {
        0.0
    };
    let score = incoming_score + recency + type_w - ttl_d - orphan_pen;
    RankedNode {
        node,
        score,
        incoming_count,
    }
}

fn recency_factor(updated_at_ms: i64, now_ms: i64) -> f32 {
    let days = ((now_ms - updated_at_ms).max(0) as f32) / (1000.0 * 60.0 * 60.0 * 24.0);
    (-(days / RANKING_RECENCY_DECAY_DAYS)).exp() * RANKING_RECENCY_SCALE
}

fn ttl_decay(node: &MemoryNode, now_ms: i64) -> f32 {
    let Some(ttl) = node.ttl_days else {
        return 0.0;
    };
    let days = ((now_ms - node.created_at).max(0) as f32) / (1000.0 * 60.0 * 60.0 * 24.0);
    let remaining = (ttl as f32) - days;
    if remaining <= 0.0 {
        return RANKING_TTL_EXPIRED_PENALTY;
    }
    if remaining < TTL_NEAR_THRESHOLD_DAYS {
        return RANKING_TTL_NEAR_PENALTY;
    }
    0.0
}
