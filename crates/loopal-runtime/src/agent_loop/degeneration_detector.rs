use chrono::{Duration, Utc};
use loopal_protocol::{DegenerationSignal, DegenerationSummary, MessageSource};

use super::degeneration_feedback::build_feedback;
use super::governance::traits::{Governance, PostTurnAction};
use super::turn_history::{TurnHistory, TurnRecord};

pub struct DegenerationDetector {
    barren_threshold: u32,
    duplicate_text_threshold: u32,
    wake_after: Duration,
    silenced_until_progress: bool,
}

impl DegenerationDetector {
    pub fn new(barren_threshold: u32, duplicate_text_threshold: u32, wake_after_secs: u32) -> Self {
        Self {
            barren_threshold: barren_threshold.max(2),
            duplicate_text_threshold: duplicate_text_threshold.max(2),
            wake_after: Duration::seconds(wake_after_secs.max(60) as i64),
            silenced_until_progress: false,
        }
    }

    fn barren_streak(&self, history: &TurnHistory) -> u32 {
        history.consecutive_trailing(|r| r.metrics.tool_calls_approved == 0) as u32
    }

    fn duplicate_run(&self, history: &TurnHistory) -> u32 {
        history.consecutive_same_text_hash() as u32
    }

    fn detect(&self, history: &TurnHistory) -> Option<(DegenerationSignal, u32)> {
        let dup = self.duplicate_run(history);
        if dup >= self.duplicate_text_threshold {
            return Some((DegenerationSignal::RepeatedText, dup));
        }
        let barren = self.barren_streak(history);
        if barren >= self.barren_threshold {
            return Some((DegenerationSignal::BarrenStreak, barren));
        }
        None
    }
}

impl Governance for DegenerationDetector {
    fn on_after_turn(&mut self, record: &TurnRecord, history: &TurnHistory) -> PostTurnAction {
        if record.metrics.tool_calls_approved > 0 {
            self.silenced_until_progress = false;
        }
        if self.silenced_until_progress {
            return PostTurnAction::None;
        }
        match self.detect(history) {
            Some((signal, count)) => {
                self.silenced_until_progress = true;
                let summary = DegenerationSummary {
                    signal,
                    count,
                    wake_deadline: Utc::now() + self.wake_after,
                };
                PostTurnAction::Degeneration {
                    summary,
                    feedback_to_model: build_feedback(signal, count),
                }
            }
            None => PostTurnAction::None,
        }
    }

    fn on_compact_completed(&mut self) {
        self.silenced_until_progress = false;
    }

    fn on_envelope_received(&mut self, source: &MessageSource) {
        // External signal (human/cron/peer-agent) clears the silenced flag
        // so a fresh degeneration window can re-emit. Without this, an
        // /unsuspend followed by relapse would be silent.
        if matches!(
            source,
            MessageSource::Human | MessageSource::Scheduled | MessageSource::Channel { .. }
        ) {
            self.silenced_until_progress = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_loop::turn_metrics::TurnMetrics;

    fn record(text_hash: Option<u64>, tool_calls_approved: u32) -> TurnRecord {
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

    fn barren_record(hash: Option<u64>) -> TurnRecord {
        record(hash, 0)
    }

    fn productive_record() -> TurnRecord {
        record(Some(0xdead_beef), 1)
    }

    fn push_barren(h: &mut TurnHistory, count: usize, hash: u64) {
        for _ in 0..count {
            h.push(barren_record(Some(hash)));
        }
    }

    #[test]
    fn no_action_when_thresholds_not_reached() {
        let mut d = DegenerationDetector::new(20, 5, 600);
        let mut h = TurnHistory::with_capacity(100);
        push_barren(&mut h, 3, 7);
        let r = barren_record(Some(7));
        h.push(r.clone());
        assert!(matches!(d.on_after_turn(&r, &h), PostTurnAction::None));
    }

    #[test]
    fn repeated_text_triggers_at_threshold() {
        let mut d = DegenerationDetector::new(50, 3, 600);
        let mut h = TurnHistory::with_capacity(100);
        push_barren(&mut h, 2, 7);
        let r = barren_record(Some(7));
        h.push(r.clone());
        match d.on_after_turn(&r, &h) {
            PostTurnAction::Degeneration { summary, .. } => {
                assert_eq!(summary.signal, DegenerationSignal::RepeatedText);
                assert_eq!(summary.count, 3);
            }
            other => panic!("expected Degeneration, got {other:?}"),
        }
    }

    #[test]
    fn barren_streak_triggers_when_no_repetition() {
        let mut d = DegenerationDetector::new(3, 100, 600);
        let mut h = TurnHistory::with_capacity(100);
        h.push(barren_record(Some(0)));
        h.push(barren_record(Some(1)));
        let r = barren_record(Some(2));
        h.push(r.clone());
        match d.on_after_turn(&r, &h) {
            PostTurnAction::Degeneration { summary, .. } => {
                assert_eq!(summary.signal, DegenerationSignal::BarrenStreak);
                assert_eq!(summary.count, 3);
            }
            other => panic!("expected Degeneration, got {other:?}"),
        }
    }

    fn assert_degen(d: &mut DegenerationDetector, h: &TurnHistory) {
        let r = h.last().cloned().unwrap();
        assert!(matches!(
            d.on_after_turn(&r, h),
            PostTurnAction::Degeneration { .. }
        ));
    }

    #[test]
    fn silenced_after_trigger_until_productive_turn() {
        let mut d = DegenerationDetector::new(50, 3, 600);
        let mut h = TurnHistory::with_capacity(100);
        push_barren(&mut h, 3, 7);
        assert_degen(&mut d, &h);
        h.push(barren_record(Some(7)));
        let r = h.last().cloned().unwrap();
        assert!(matches!(d.on_after_turn(&r, &h), PostTurnAction::None));
        let p = productive_record();
        h.push(p.clone());
        assert!(matches!(d.on_after_turn(&p, &h), PostTurnAction::None));
        push_barren(&mut h, 3, 9);
        assert_degen(&mut d, &h);
    }

    #[test]
    fn silence_resets_by_compact_and_external_envelope() {
        let mut d = DegenerationDetector::new(50, 3, 600);
        let mut h = TurnHistory::with_capacity(100);
        push_barren(&mut h, 3, 7);
        assert_degen(&mut d, &h);
        d.on_compact_completed();
        assert_degen(&mut d, &h);
        d.on_envelope_received(&MessageSource::Human);
        assert_degen(&mut d, &h);
        let r = h.last().cloned().unwrap();
        d.on_envelope_received(&MessageSource::System("goal_continuation".into()));
        assert!(matches!(d.on_after_turn(&r, &h), PostTurnAction::None));
    }
}
