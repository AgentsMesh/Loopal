use crate::store::types::MemoryKind;

pub const RECALL_RECENCY_DECAY_DAYS: f32 = 90.0;
pub const RECALL_RECENCY_BONUS_SCALE: f32 = 0.2;
pub const RECALL_TTL_EXPIRED_PENALTY: f32 = 0.5;
pub const RECALL_TTL_NEAR_PENALTY: f32 = 0.2;

pub const RANKING_RECENCY_DECAY_DAYS: f32 = 60.0;
pub const RANKING_RECENCY_SCALE: f32 = 0.5;
pub const RANKING_TTL_EXPIRED_PENALTY: f32 = 1.0;
pub const RANKING_TTL_NEAR_PENALTY: f32 = 0.3;
pub const RANKING_ORPHAN_PENALTY: f32 = 0.3;

pub const TTL_NEAR_THRESHOLD_DAYS: f32 = 14.0;

pub const RECALL_REINFORCEMENT_SCALE: f32 = 0.15;
pub const IMPORTANCE_SCALE: f32 = 0.20;

pub fn recall_kind_weight(kind: MemoryKind) -> f32 {
    match kind {
        MemoryKind::Project => 1.2,
        MemoryKind::Feedback => 1.1,
        MemoryKind::User => 1.0,
        MemoryKind::Index => 0.9,
        MemoryKind::Reference => 0.8,
    }
}

pub fn ranking_kind_weight(kind: MemoryKind) -> f32 {
    match kind {
        MemoryKind::Index => 0.0,
        other => recall_kind_weight(other),
    }
}
