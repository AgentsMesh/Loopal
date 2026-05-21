use chrono::{DateTime, Utc};
use loopal_protocol::{ContinuationGateSummary, GateCloseReason};

/// Closure intent for `ContinuationGate::close`. Encodes the *invariant*
/// that a non-`UserSuspend` close MUST carry a `wake_at` deadline; the type
/// system rejects calls that violate this (cf. arch review W1).
pub enum GateClose {
    ModelRequested { wake_at: DateTime<Utc> },
    Degeneration { wake_at: DateTime<Utc> },
    UserSuspend,
}

pub struct ContinuationGate {
    open: bool,
    closed_reason: Option<GateCloseReason>,
    wake_deadline: Option<DateTime<Utc>>,
}

impl ContinuationGate {
    pub fn new() -> Self {
        Self {
            open: true,
            closed_reason: None,
            wake_deadline: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn closed_reason(&self) -> Option<GateCloseReason> {
        self.closed_reason
    }

    pub fn wake_deadline(&self) -> Option<DateTime<Utc>> {
        self.wake_deadline
    }

    pub fn close(&mut self, intent: GateClose) {
        self.open = false;
        match intent {
            GateClose::ModelRequested { wake_at } => {
                self.closed_reason = Some(GateCloseReason::ModelRequested);
                self.wake_deadline = Some(wake_at);
            }
            GateClose::Degeneration { wake_at } => {
                self.closed_reason = Some(GateCloseReason::Degeneration);
                self.wake_deadline = Some(wake_at);
            }
            GateClose::UserSuspend => {
                self.closed_reason = Some(GateCloseReason::UserSuspend);
                self.wake_deadline = None;
            }
        }
    }

    pub fn open_for_envelope(&mut self) {
        self.open = true;
        self.closed_reason = None;
        self.wake_deadline = None;
    }

    pub fn maybe_open_on_deadline(&mut self, now: DateTime<Utc>) -> bool {
        if self.open {
            return false;
        }
        let Some(deadline) = self.wake_deadline else {
            return false;
        };
        if now < deadline {
            return false;
        }
        self.open = true;
        self.closed_reason = Some(GateCloseReason::IdleTimeout);
        self.wake_deadline = None;
        true
    }

    pub fn summary(&self) -> ContinuationGateSummary {
        ContinuationGateSummary {
            open: self.open,
            closed_reason: self.closed_reason,
            wake_deadline: self.wake_deadline,
        }
    }
}

impl Default for ContinuationGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn new_gate_is_open() {
        let g = ContinuationGate::new();
        assert!(g.is_open());
        assert!(g.closed_reason().is_none());
        assert!(g.wake_deadline().is_none());
    }

    #[test]
    fn close_model_requested_sets_reason_and_wake() {
        let mut g = ContinuationGate::new();
        let when = Utc::now() + Duration::seconds(120);
        g.close(GateClose::ModelRequested { wake_at: when });
        assert!(!g.is_open());
        assert_eq!(g.closed_reason(), Some(GateCloseReason::ModelRequested));
        assert_eq!(g.wake_deadline(), Some(when));
    }

    #[test]
    fn close_degeneration_sets_reason_and_wake() {
        let mut g = ContinuationGate::new();
        let when = Utc::now() + Duration::seconds(3600);
        g.close(GateClose::Degeneration { wake_at: when });
        assert_eq!(g.closed_reason(), Some(GateCloseReason::Degeneration));
        assert_eq!(g.wake_deadline(), Some(when));
    }

    #[test]
    fn close_user_suspend_carries_no_deadline() {
        let mut g = ContinuationGate::new();
        g.close(GateClose::UserSuspend);
        assert!(!g.is_open());
        assert_eq!(g.closed_reason(), Some(GateCloseReason::UserSuspend));
        assert!(g.wake_deadline().is_none());
    }

    #[test]
    fn open_for_envelope_clears_reason_and_deadline() {
        let mut g = ContinuationGate::new();
        g.close(GateClose::Degeneration {
            wake_at: Utc::now() + Duration::seconds(3600),
        });
        g.open_for_envelope();
        assert!(g.is_open());
        assert!(g.closed_reason().is_none());
        assert!(g.wake_deadline().is_none());
    }

    #[test]
    fn maybe_open_on_deadline_noop_when_open() {
        let mut g = ContinuationGate::new();
        assert!(!g.maybe_open_on_deadline(Utc::now()));
    }

    #[test]
    fn maybe_open_on_deadline_noop_before_deadline() {
        let mut g = ContinuationGate::new();
        let when = Utc::now() + Duration::seconds(120);
        g.close(GateClose::ModelRequested { wake_at: when });
        assert!(!g.maybe_open_on_deadline(Utc::now()));
        assert!(!g.is_open());
    }

    #[test]
    fn maybe_open_on_deadline_reopens_at_or_after_deadline() {
        let mut g = ContinuationGate::new();
        let when = Utc::now() - Duration::seconds(1);
        g.close(GateClose::ModelRequested { wake_at: when });
        assert!(g.maybe_open_on_deadline(Utc::now()));
        assert!(g.is_open());
        assert_eq!(g.closed_reason(), Some(GateCloseReason::IdleTimeout));
    }

    #[test]
    fn maybe_open_on_deadline_ignored_for_user_suspend() {
        let mut g = ContinuationGate::new();
        g.close(GateClose::UserSuspend);
        assert!(!g.maybe_open_on_deadline(Utc::now() + Duration::seconds(86_400)));
        assert!(!g.is_open());
    }

    #[test]
    fn summary_round_trips_state() {
        let mut g = ContinuationGate::new();
        let when = Utc::now() + Duration::seconds(600);
        g.close(GateClose::Degeneration { wake_at: when });
        let s = g.summary();
        assert!(!s.open);
        assert_eq!(s.closed_reason, Some(GateCloseReason::Degeneration));
        assert_eq!(s.wake_deadline, Some(when));
    }
}
