use loopal_protocol::Question;

use crate::conversation::{ClassifierStatus, PendingQuestion};
use crate::state::SessionViewState;

pub(super) fn user_question_request(
    state: &mut SessionViewState,
    id: &str,
    questions: &[Question],
    classifier_running: bool,
) -> bool {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.pending_question = Some(
        PendingQuestion::new(id.to_string(), questions.to_vec())
            .with_classifier_running(classifier_running),
    );
    true
}

pub(super) fn user_question_resolved(state: &mut SessionViewState, id: &str) -> bool {
    let pending = &mut state.agent.conversation.pending_question;
    if pending.as_ref().is_some_and(|q| q.id == id) {
        *pending = None;
        true
    } else {
        false
    }
}

pub(super) fn classifier_progress(state: &mut SessionViewState, id: &str, elapsed_ms: u64) -> bool {
    let pending = match state.agent.conversation.pending_question.as_mut() {
        Some(p) if p.id == id => p,
        _ => return false,
    };
    if matches!(
        pending.classifier_status,
        ClassifierStatus::Failed { .. } | ClassifierStatus::Completed { .. }
    ) {
        return false;
    }
    pending.classifier_status = ClassifierStatus::Running { elapsed_ms };
    true
}

pub(super) fn classifier_failed(state: &mut SessionViewState, id: &str, reason: &str) -> bool {
    let pending = match state.agent.conversation.pending_question.as_mut() {
        Some(p) if p.id == id => p,
        _ => return false,
    };
    // reason: terminal status (Failed/Completed) must not be overwritten by
    // a late or out-of-order event; this protects the UI from flipping back
    // and forth if events arrive in unexpected order.
    if matches!(
        pending.classifier_status,
        ClassifierStatus::Failed { .. } | ClassifierStatus::Completed { .. }
    ) {
        return false;
    }
    pending.classifier_status = ClassifierStatus::Failed {
        reason: reason.to_string(),
    };
    true
}

pub(super) fn classifier_completed(
    state: &mut SessionViewState,
    id: &str,
    answers: &[String],
) -> bool {
    let pending = match state.agent.conversation.pending_question.as_mut() {
        Some(p) if p.id == id => p,
        _ => return false,
    };
    if matches!(
        pending.classifier_status,
        ClassifierStatus::Failed { .. } | ClassifierStatus::Completed { .. }
    ) {
        return false;
    }
    pending.classifier_status = ClassifierStatus::Completed {
        answers: answers.to_vec(),
    };
    true
}
