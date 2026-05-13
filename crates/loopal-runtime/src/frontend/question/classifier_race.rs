use std::time::Instant;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use loopal_classifier::QuestionResult;
use loopal_protocol::{AgentEventPayload, Question, ResolveSource, UserQuestionResponse};

use super::super::question_handler::QuestionOutcome;
use super::classifier::ClassifierQuestionHandler;
use super::classifier_race_spawn::{
    flatten, spawn_classifier, spawn_fallback, spawn_progress_ticker,
};
use super::outraced_telemetry;

pub(super) async fn run_race(
    handler: &ClassifierQuestionHandler,
    question_id: String,
    questions: Vec<Question>,
) -> QuestionOutcome {
    let cancel = CancellationToken::new();
    let start = Instant::now();
    let mut manual_handle = spawn_fallback(handler, &question_id, questions.clone());
    let mut classifier_handle = spawn_classifier(handler, questions, cancel.clone());
    let ticker_handle = spawn_progress_ticker(handler, question_id.clone(), start, cancel.clone());
    let winner = tokio::select! {
        biased;
        m = &mut manual_handle => RaceWinner::Manual(m),
        c = &mut classifier_handle => RaceWinner::Classifier(c),
    };
    ticker_handle.abort();
    match winner {
        RaceWinner::Manual(m) => on_manual_first(handler, question_id, m, classifier_handle).await,
        RaceWinner::Classifier(c) => {
            on_classifier_first(handler, question_id, c, manual_handle, cancel).await
        }
    }
}

enum RaceWinner {
    Manual(Result<QuestionOutcome, tokio::task::JoinError>),
    Classifier(Result<Result<(QuestionResult, usize), String>, tokio::task::JoinError>),
}

async fn on_manual_first(
    handler: &ClassifierQuestionHandler,
    question_id: String,
    result: Result<QuestionOutcome, tokio::task::JoinError>,
    classifier_handle: JoinHandle<Result<(QuestionResult, usize), String>>,
) -> QuestionOutcome {
    let outcome = result
        .unwrap_or_else(|_| QuestionOutcome::cancelled(&question_id, "fallback task panicked"));
    let manual_answers = outraced_telemetry::extract_answers(&outcome.response);
    let manual_duration = outcome.duration_ms;
    let qid_clone = question_id.clone();
    let classifier = handler.classifier.clone();
    // reason: detached task — telemetry is best-effort. If the session ends
    // before this task awaits classifier_handle, the row is dropped. Acceptable
    // because (a) telemetry shouldn't block ask() returning, (b) the
    // ClassifierEngine has no Drop side-effects to clean up today.
    tokio::spawn(async move {
        let cls = classifier_handle.await;
        if let Ok(Ok((qr, _))) = cls
            && qr.error.is_none()
            && !qr.answers.is_empty()
        {
            let cls_answers: Vec<String> =
                qr.answers.iter().map(|labels| labels.join(", ")).collect();
            outraced_telemetry::record_outraced(outraced_telemetry::OutracedInput {
                question_id: &qid_clone,
                manual_answers: &manual_answers,
                classifier_answers: &cls_answers,
                manual_duration_ms: manual_duration,
                classifier_duration_ms: qr.duration_ms,
            });
            classifier.on_outraced("@question");
        }
    });
    outcome
}

async fn on_classifier_first(
    handler: &ClassifierQuestionHandler,
    question_id: String,
    result: Result<Result<(QuestionResult, usize), String>, tokio::task::JoinError>,
    manual_handle: JoinHandle<QuestionOutcome>,
    cancel: CancellationToken,
) -> QuestionOutcome {
    let (cls, expected) = match flatten(result) {
        Ok(v) => v,
        Err(reason) => return defer_to_manual(handler, &question_id, reason, manual_handle).await,
    };
    if let Some(err) = cls.error.clone() {
        return defer_to_manual(handler, &question_id, err, manual_handle).await;
    }
    if cls.answers.len() != expected {
        warn!(
            expected,
            got = cls.answers.len(),
            "classifier answer count mismatch"
        );
        return defer_to_manual(
            handler,
            &question_id,
            "answer count mismatch".into(),
            manual_handle,
        )
        .await;
    }
    // reason: any empty inner array means "I won't guess this one" — treat
    // the whole call as abstain so the user makes the final call. We don't
    // partially answer (some auto, some manual) — single source per request
    // keeps the UX simpler.
    if cls.answers.iter().any(|inner| inner.is_empty()) {
        return defer_to_manual(
            handler,
            &question_id,
            "classifier abstained (subjective preference)".into(),
            manual_handle,
        )
        .await;
    }
    commit_classifier(handler, question_id, cls, manual_handle, cancel).await
}

async fn commit_classifier(
    handler: &ClassifierQuestionHandler,
    question_id: String,
    cls: QuestionResult,
    manual_handle: JoinHandle<QuestionOutcome>,
    cancel: CancellationToken,
) -> QuestionOutcome {
    cancel.cancel();
    manual_handle.abort();
    let flat: Vec<String> = cls.answers.iter().map(|labels| labels.join(", ")).collect();
    handler
        .emitter
        .emit_best_effort(
            AgentEventPayload::ClassifierCompleted {
                id: question_id.clone(),
                answers: flat.clone(),
                duration_ms: cls.duration_ms,
            },
            "classifier_race::commit_classifier::ClassifierCompleted",
        )
        .await;
    handler
        .emitter
        .emit_best_effort(
            AgentEventPayload::UserQuestionResolved {
                id: question_id.clone(),
                by: ResolveSource::Classifier,
            },
            "classifier_race::commit_classifier::UserQuestionResolved",
        )
        .await;
    QuestionOutcome::classifier(
        UserQuestionResponse::answered(&question_id, flat),
        cls.reason,
        cls.duration_ms,
    )
}

async fn defer_to_manual(
    handler: &ClassifierQuestionHandler,
    question_id: &str,
    reason: String,
    manual_handle: JoinHandle<QuestionOutcome>,
) -> QuestionOutcome {
    handler
        .emitter
        .emit_best_effort(
            AgentEventPayload::ClassifierFailed {
                id: question_id.to_string(),
                reason,
            },
            "classifier_race::defer_to_manual::ClassifierFailed",
        )
        .await;
    manual_handle
        .await
        .unwrap_or_else(|_| QuestionOutcome::cancelled(question_id, "fallback task panicked"))
}
