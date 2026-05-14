use loopal_protocol::Question;

use crate::conversation::{ClassifierStatus, PendingQuestion};
use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn user_question_request(
    state: &mut SessionViewState,
    id: &str,
    questions: &[Question],
    classifier_running: bool,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.pending_question = Some(
        PendingQuestion::new(id.to_string(), questions.to_vec())
            .with_classifier_running(classifier_running),
    );
    MutationEffect::Mutated
}

pub(super) fn user_question_resolved(state: &mut SessionViewState, id: &str) -> MutationEffect {
    let pending = &mut state.agent.conversation.pending_question;
    if pending.as_ref().is_some_and(|q| q.id == id) {
        *pending = None;
        MutationEffect::Mutated
    } else {
        MutationEffect::NoOp
    }
}

pub(super) fn classifier_progress(
    state: &mut SessionViewState,
    id: &str,
    elapsed_ms: u64,
) -> MutationEffect {
    let pending = match state.agent.conversation.pending_question.as_mut() {
        Some(p) if p.id == id => p,
        _ => return MutationEffect::NoOp,
    };
    if matches!(
        pending.classifier_status,
        ClassifierStatus::Failed { .. } | ClassifierStatus::Completed { .. }
    ) {
        return MutationEffect::NoOp;
    }
    pending.classifier_status = ClassifierStatus::Running { elapsed_ms };
    MutationEffect::Mutated
}

pub(super) fn classifier_failed(
    state: &mut SessionViewState,
    id: &str,
    reason: &str,
) -> MutationEffect {
    let pending = match state.agent.conversation.pending_question.as_mut() {
        Some(p) if p.id == id => p,
        _ => return MutationEffect::NoOp,
    };
    // reason: terminal status (Failed/Completed) must not be overwritten by
    // a late or out-of-order event; this protects the UI from flipping back
    // and forth if events arrive in unexpected order.
    if matches!(
        pending.classifier_status,
        ClassifierStatus::Failed { .. } | ClassifierStatus::Completed { .. }
    ) {
        return MutationEffect::NoOp;
    }
    pending.classifier_status = ClassifierStatus::Failed {
        reason: reason.to_string(),
    };
    MutationEffect::Mutated
}

pub(super) fn classifier_completed(
    state: &mut SessionViewState,
    id: &str,
    answers: &[String],
) -> MutationEffect {
    let pending = match state.agent.conversation.pending_question.as_mut() {
        Some(p) if p.id == id => p,
        _ => return MutationEffect::NoOp,
    };
    if matches!(
        pending.classifier_status,
        ClassifierStatus::Failed { .. } | ClassifierStatus::Completed { .. }
    ) {
        return MutationEffect::NoOp;
    }
    pending.classifier_status = ClassifierStatus::Completed {
        answers: answers.to_vec(),
    };
    MutationEffect::Mutated
}
