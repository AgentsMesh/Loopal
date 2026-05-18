use tracing::debug;

use super::runner::AgentLoopRunner;
use super::turn_metrics::TurnMetrics;

pub(crate) fn compute_next_barren_count(
    was_continuation: bool,
    productive: bool,
    current: u32,
) -> u32 {
    if !was_continuation {
        return 0;
    }
    if productive {
        0
    } else {
        current.saturating_add(1)
    }
}

impl AgentLoopRunner {
    pub(super) fn record_turn_for_barren_tracking(&mut self, metrics: &TurnMetrics) {
        let was_continuation = self.last_continuation_goal_id.take().is_some();
        let productive = metrics.tool_calls_approved > 0;
        let next =
            compute_next_barren_count(was_continuation, productive, self.barren_continuation_count);
        if next > self.barren_continuation_count {
            debug!(count = next, "barren continuation observed");
        }
        self.barren_continuation_count = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_driven_turn_resets_counter() {
        assert_eq!(compute_next_barren_count(false, false, 5), 0);
        assert_eq!(compute_next_barren_count(false, true, 5), 0);
    }

    #[test]
    fn productive_continuation_resets_counter() {
        assert_eq!(compute_next_barren_count(true, true, 1), 0);
        assert_eq!(compute_next_barren_count(true, true, 0), 0);
    }

    #[test]
    fn barren_continuation_increments_counter() {
        assert_eq!(compute_next_barren_count(true, false, 0), 1);
        assert_eq!(compute_next_barren_count(true, false, 1), 2);
        assert_eq!(compute_next_barren_count(true, false, 2), 3);
    }

    #[test]
    fn barren_counter_saturates_at_u32_max() {
        assert_eq!(compute_next_barren_count(true, false, u32::MAX), u32::MAX);
    }
}
