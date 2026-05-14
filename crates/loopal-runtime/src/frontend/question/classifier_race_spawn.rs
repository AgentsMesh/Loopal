use std::time::Instant;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use loopal_classifier::QuestionResult;
use loopal_protocol::event_id::propagate_to_spawn;
use loopal_protocol::{AgentEventPayload, Question};

use super::super::question_handler::{AskOptions, QuestionOutcome};
use super::classifier::ClassifierQuestionHandler;

pub(super) fn spawn_fallback(
    handler: &ClassifierQuestionHandler,
    question_id: &str,
    questions: Vec<Question>,
) -> JoinHandle<QuestionOutcome> {
    let fallback = handler.fallback.clone();
    let opts = AskOptions {
        id: question_id.to_string(),
        classifier_running: true,
    };
    tokio::spawn(propagate_to_spawn(async move {
        fallback.ask_with_options(questions, opts).await
    }))
}

pub(super) fn spawn_classifier(
    handler: &ClassifierQuestionHandler,
    questions: Vec<Question>,
    cancel: CancellationToken,
) -> JoinHandle<Result<(QuestionResult, usize), String>> {
    let ctx = handler.classifier_ctx();
    tokio::spawn(propagate_to_spawn(async move {
        tokio::select! {
            r = ctx.run(questions) => r,
            _ = cancel.cancelled() => Err("cancelled by race".into()),
        }
    }))
}

pub(super) fn spawn_progress_ticker(
    handler: &ClassifierQuestionHandler,
    question_id: String,
    start: Instant,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let emitter = handler.emitter.clone();
    let interval = handler.progress_interval;
    tokio::spawn(propagate_to_spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    emitter
                        .emit_best_effort(
                            AgentEventPayload::ClassifierProgress {
                                id: question_id.clone(),
                                elapsed_ms,
                            },
                            "frontend::question::classifier_progress",
                        )
                        .await;
                }
                _ = cancel.cancelled() => break,
            }
        }
    }))
}

pub(super) fn flatten(
    result: Result<Result<(QuestionResult, usize), String>, tokio::task::JoinError>,
) -> Result<(QuestionResult, usize), String> {
    match result {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(reason)) => Err(reason),
        Err(e) => Err(format!("panic: {e}")),
    }
}
