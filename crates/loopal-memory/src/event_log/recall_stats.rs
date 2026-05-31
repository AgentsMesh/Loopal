use std::collections::HashMap;

use crate::event_log::schema::{Event, EventKind};

pub type RecallStatsMap = HashMap<String, RecallStats>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecallStats {
    pub recall_count: u32,
    pub last_recalled_at: i64,
    pub importance: i8,
    pub importance_ts: i64,
}

impl RecallStats {
    pub fn fold_event(&mut self, ev: &Event) {
        match &ev.kind {
            EventKind::RecallHit { .. } => {
                self.recall_count = self.recall_count.saturating_add(1);
                if ev.ts > self.last_recalled_at {
                    self.last_recalled_at = ev.ts;
                }
            }
            EventKind::ImportanceTag { importance, .. } => {
                if ev.ts >= self.importance_ts {
                    self.importance_ts = ev.ts;
                    self.importance = *importance;
                }
            }
            _ => {}
        }
    }
}

pub fn apply_events_to_map(map: &mut RecallStatsMap, events: &[Event]) {
    for ev in events {
        let Some(slug) = ev.node_slug() else {
            continue;
        };
        let entry = map.entry(slug.to_string()).or_default();
        entry.fold_event(ev);
    }
}
