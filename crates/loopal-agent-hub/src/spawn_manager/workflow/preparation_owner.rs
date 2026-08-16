use std::sync::atomic::{AtomicU8, Ordering};

use loopal_protocol::WorkflowPermissionCausation;

pub(super) struct PreparationOwner {
    pub(super) causation: WorkflowPermissionCausation,
    state: AtomicU8,
}

impl PreparationOwner {
    const OPEN: u8 = 0;
    const FORKING: u8 = 1;
    const CANCELLED_BEFORE_FORK: u8 = 2;
    const CANCELLED_AFTER_FORK: u8 = 3;

    pub(super) fn new(causation: WorkflowPermissionCausation) -> Self {
        Self {
            causation,
            state: AtomicU8::new(Self::OPEN),
        }
    }

    pub(super) fn claim_fork(&self) -> Result<(), String> {
        self.state
            .compare_exchange(
                Self::OPEN,
                Self::FORKING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| "workflow preparation was cancelled".into())
    }

    pub(super) fn cancel(&self) {
        let _ = self
            .state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| match state {
                Self::OPEN => Some(Self::CANCELLED_BEFORE_FORK),
                Self::FORKING => Some(Self::CANCELLED_AFTER_FORK),
                _ => None,
            });
    }

    pub(super) fn is_cancelled(&self) -> bool {
        matches!(
            self.state.load(Ordering::Acquire),
            Self::CANCELLED_BEFORE_FORK | Self::CANCELLED_AFTER_FORK
        )
    }
}
