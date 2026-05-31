use chrono::Utc;

use crate::event_log::RecallStats;
use crate::policy::{
    IMPORTANCE_SCALE, RECALL_RECENCY_BONUS_SCALE, RECALL_RECENCY_DECAY_DAYS,
    RECALL_REINFORCEMENT_SCALE, RECALL_TTL_EXPIRED_PENALTY, RECALL_TTL_NEAR_PENALTY,
    TTL_NEAR_THRESHOLD_DAYS, recall_kind_weight,
};
use crate::store::types::{EdgeKind, MemoryEdge, MemoryKind, MemoryNode};

pub fn fts_score(rank: f32) -> f32 {
    -rank
}

pub fn neighbor_decay(distance: u32) -> f32 {
    if distance == 0 {
        return 1.0;
    }
    0.5_f32.powi(distance as i32)
}

pub fn recall_edge_weight(kind: EdgeKind) -> f32 {
    match kind {
        EdgeKind::References => 1.0,
        EdgeKind::ContainedIn => 0.9,
        EdgeKind::DerivedFrom => 0.8,
        EdgeKind::SupersededBy => 0.7,
        EdgeKind::CoOccursSlug => 0.55,
        EdgeKind::CoOccursToken => 0.45,
        EdgeKind::Contradicts => 0.6,
    }
}

pub fn recall_type_weight(kind: MemoryKind) -> f32 {
    recall_kind_weight(kind)
}

pub fn recall_recency_bonus(updated_at_ms: i64, stats: Option<&RecallStats>) -> f32 {
    let anchor_ts = stats
        .map(|s| s.last_recalled_at.max(updated_at_ms))
        .unwrap_or(updated_at_ms);
    let now = Utc::now().timestamp_millis();
    let days = ((now - anchor_ts).max(0) as f32) / (1000.0 * 60.0 * 60.0 * 24.0);
    (-(days / RECALL_RECENCY_DECAY_DAYS)).exp() * RECALL_RECENCY_BONUS_SCALE
}

pub fn recall_ttl_penalty(node: &MemoryNode) -> f32 {
    let Some(ttl) = node.ttl_days else {
        return 0.0;
    };
    let now = Utc::now().timestamp_millis();
    let days = ((now - node.created_at).max(0) as f32) / (1000.0 * 60.0 * 60.0 * 24.0);
    let remaining = (ttl as f32) - days;
    if remaining <= 0.0 {
        return RECALL_TTL_EXPIRED_PENALTY;
    }
    if remaining < TTL_NEAR_THRESHOLD_DAYS {
        return RECALL_TTL_NEAR_PENALTY;
    }
    0.0
}

pub fn co_occurrence_bonus(edge: &MemoryEdge) -> f32 {
    edge.confidence * 0.3
}

pub fn recall_reinforcement_bonus(stats: Option<&RecallStats>) -> f32 {
    let Some(s) = stats else {
        return 0.0;
    };
    (1.0_f32 + s.recall_count as f32).ln() * RECALL_REINFORCEMENT_SCALE
}

pub fn importance_bonus(stats: Option<&RecallStats>) -> f32 {
    let Some(s) = stats else {
        return 0.0;
    };
    s.importance as f32 * IMPORTANCE_SCALE
}
