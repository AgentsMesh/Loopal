use super::super::runner::AgentLoopRunner;

#[must_use]
pub(super) enum CompactionHostGuard {
    Reopened(loopal_turn::TurnId),
    SyntheticOpened,
    PreExisting,
    AlreadySummarized,
    OpenFailed,
}

impl AgentLoopRunner {
    pub(super) fn ensure_compaction_host_turn(&mut self) -> CompactionHostGuard {
        if self.turns.current_turn_id().is_some() {
            return CompactionHostGuard::PreExisting;
        }
        if let Some(id) = self.turns.reopen_last_completed_turn() {
            return CompactionHostGuard::Reopened(id);
        }
        if !self.has_unsummarized_new_content_after_last_summary()
            && self.turns.store().turns().iter().any(|t| {
                t.body
                    .steps
                    .iter()
                    .any(|s| matches!(s, loopal_turn::TurnStep::CompactionSummary(_)))
            })
        {
            return CompactionHostGuard::AlreadySummarized;
        }
        match self.start_turn_record(loopal_turn::TurnTrigger::Resume) {
            Some(_) => CompactionHostGuard::SyntheticOpened,
            None => CompactionHostGuard::OpenFailed,
        }
    }

    fn has_unsummarized_new_content_after_last_summary(&self) -> bool {
        let turns = self.turns.store().turns();
        let latest_summary_pos = turns.iter().rposition(|t| {
            t.body
                .steps
                .iter()
                .any(|s| matches!(s, loopal_turn::TurnStep::CompactionSummary(_)))
        });
        let Some(summary_turn_idx) = latest_summary_pos else {
            return true;
        };
        turns.iter().skip(summary_turn_idx + 1).any(|t| {
            t.body.steps.iter().any(|s| {
                matches!(
                    s,
                    loopal_turn::TurnStep::LlmCall { .. } | loopal_turn::TurnStep::ToolBatch(_)
                )
            })
        })
    }

    pub(super) fn close_compaction_host(
        &mut self,
        guard: CompactionHostGuard,
        summary_appended: bool,
    ) {
        match guard {
            CompactionHostGuard::SyntheticOpened => {
                let outcome = if summary_appended {
                    loopal_turn::TurnOutcome::Complete
                } else {
                    loopal_turn::TurnOutcome::Cancelled {
                        cause: loopal_turn::CancelledCause::ParentTurnAborted,
                    }
                };
                self.end_turn_record(outcome);
            }
            CompactionHostGuard::Reopened(id) => {
                if summary_appended {
                    self.turns.close_reopened_silently(&id);
                } else {
                    self.turns.rollback_reopen(&id);
                }
            }
            CompactionHostGuard::PreExisting
            | CompactionHostGuard::OpenFailed
            | CompactionHostGuard::AlreadySummarized => {}
        }
    }
}
