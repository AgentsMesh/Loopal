use std::collections::VecDeque;

use super::turn_metrics::TurnMetrics;

#[derive(Debug, Clone)]
pub struct TurnRecord {
    pub metrics: TurnMetrics,
    pub text_hash: Option<u64>,
}

const DEFAULT_CAPACITY: usize = 64;

pub struct TurnHistory {
    records: VecDeque<TurnRecord>,
    capacity: usize,
}

impl TurnHistory {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            records: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, record: TurnRecord) {
        if self.records.len() >= self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn last(&self) -> Option<&TurnRecord> {
        self.records.back()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TurnRecord> {
        self.records.iter()
    }

    pub fn consecutive_trailing<F: Fn(&TurnRecord) -> bool>(&self, pred: F) -> usize {
        self.records.iter().rev().take_while(|r| pred(r)).count()
    }

    /// Count of trailing turns sharing the most recent (non-`None`) text
    /// hash. Returns 0 when the most recent record has `text_hash: None`
    /// — this is intentional: a turn without text output cannot start a
    /// repetition run, so the detector should not treat it as one.
    pub fn consecutive_same_text_hash(&self) -> usize {
        let mut iter = self.records.iter().rev();
        let head_hash = match iter.next().and_then(|r| r.text_hash) {
            Some(h) => h,
            None => return 0,
        };
        let mut count = 1usize;
        for r in iter {
            if r.text_hash == Some(head_hash) {
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl Default for TurnHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(text_hash: Option<u64>, tool_calls_approved: u32) -> TurnRecord {
        let m = TurnMetrics {
            tool_calls_approved,
            text_hash,
            ..Default::default()
        };
        TurnRecord {
            metrics: m,
            text_hash,
        }
    }

    #[test]
    fn new_history_is_empty() {
        let h = TurnHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn push_increments_until_capacity_then_drops_oldest() {
        let mut h = TurnHistory::with_capacity(3);
        h.push(rec(Some(1), 0));
        h.push(rec(Some(2), 0));
        h.push(rec(Some(3), 0));
        h.push(rec(Some(4), 0));
        assert_eq!(h.len(), 3);
        let hashes: Vec<_> = h.iter().map(|r| r.text_hash).collect();
        assert_eq!(hashes, vec![Some(2), Some(3), Some(4)]);
    }

    #[test]
    fn consecutive_trailing_counts_matching_tail() {
        let mut h = TurnHistory::with_capacity(10);
        h.push(rec(Some(1), 3));
        h.push(rec(Some(2), 0));
        h.push(rec(Some(3), 0));
        h.push(rec(Some(4), 0));
        let count = h.consecutive_trailing(|r| r.metrics.tool_calls_approved == 0);
        assert_eq!(count, 3);
    }

    #[test]
    fn consecutive_trailing_stops_at_first_mismatch() {
        let mut h = TurnHistory::with_capacity(10);
        h.push(rec(Some(1), 0));
        h.push(rec(Some(2), 1));
        h.push(rec(Some(3), 0));
        let count = h.consecutive_trailing(|r| r.metrics.tool_calls_approved == 0);
        assert_eq!(count, 1);
    }

    #[test]
    fn consecutive_same_text_hash_counts_trailing_runs() {
        let mut h = TurnHistory::with_capacity(10);
        h.push(rec(Some(42), 0));
        h.push(rec(Some(7), 0));
        h.push(rec(Some(7), 0));
        h.push(rec(Some(7), 0));
        assert_eq!(h.consecutive_same_text_hash(), 3);
    }

    #[test]
    fn consecutive_same_text_hash_breaks_on_none() {
        let mut h = TurnHistory::with_capacity(10);
        h.push(rec(Some(7), 0));
        h.push(None.map_or_else(|| rec(None, 0), |_: u64| rec(None, 0)));
        h.push(rec(Some(7), 0));
        h.push(rec(Some(7), 0));
        assert_eq!(h.consecutive_same_text_hash(), 2);
    }

    #[test]
    fn consecutive_same_text_hash_returns_zero_when_last_is_none() {
        let mut h = TurnHistory::with_capacity(10);
        h.push(rec(Some(7), 0));
        h.push(rec(None, 0));
        assert_eq!(h.consecutive_same_text_hash(), 0);
    }

    #[test]
    fn clear_resets_state() {
        let mut h = TurnHistory::with_capacity(5);
        h.push(rec(Some(1), 0));
        h.push(rec(Some(2), 0));
        h.clear();
        assert!(h.is_empty());
    }

    #[test]
    fn zero_capacity_is_promoted_to_one() {
        let mut h = TurnHistory::with_capacity(0);
        h.push(rec(Some(1), 0));
        h.push(rec(Some(2), 0));
        assert_eq!(h.len(), 1);
    }
}
