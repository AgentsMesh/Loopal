use crate::reducer::WorkflowRevisionGap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MutationEffect {
    NoOp,
    Mutated,
    MutatedEndedTurn,
    WorkflowRevisionGap(WorkflowRevisionGap),
}

impl MutationEffect {
    pub fn changed(&self) -> bool {
        matches!(self, Self::Mutated | Self::MutatedEndedTurn)
    }

    pub fn requires_turn_end_reconcile(&self) -> bool {
        matches!(self, Self::MutatedEndedTurn)
    }
}
