use std::sync::Arc;
use std::time::Duration;

use loopal_classifier::ClassifierEngine;
use loopal_protocol::{
    AgentEventPayload, Question, QuestionOption, ResolveSource, UserQuestionResponse,
};
use loopal_runtime::frontend::question_handler::QuestionHandler;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::frontend::{ClassifierQuestionHandler, DecisionContext};

use super::classifier_question_handler_support::{
    DelayedFallback, RecordingEmitter, ScriptedProvider, StubResolver,
};

fn one_question() -> Question {
    Question {
        question: "Proceed?".into(),
        options: vec![
            QuestionOption {
                label: "yes".into(),
                description: "".into(),
            },
            QuestionOption {
                label: "no".into(),
                description: "".into(),
            },
        ],
        allow_multiple: false,
        header: None,
    }
}

#[tokio::test]
async fn classifier_parse_failure_defers_to_manual() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(150),
        UserQuestionResponse::answered("manual-id", vec!["yes".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let provider = ScriptedProvider::returning("not valid json");
    let auto = ClassifierQuestionHandler::new(
        classifier,
        fb.clone() as Arc<dyn QuestionHandler>,
        Arc::new(StubResolver {
            provider,
            model: "x".into(),
        }),
        DecisionContext::with_cwd("/tmp"),
        emitter.clone() as Arc<dyn EventEmitter>,
    );
    let outcome = auto.ask(vec![one_question()]).await;
    assert_eq!(outcome.source, ResolveSource::Manual);
    assert!(emitter.count_kind(|p| matches!(p, AgentEventPayload::ClassifierFailed { .. })) >= 1);
}

#[tokio::test]
async fn answer_count_mismatch_defers_to_manual() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(150),
        UserQuestionResponse::answered("manual-id", vec!["yes".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let provider = ScriptedProvider::returning(r#"{"answers":[["yes"]],"reason":"incomplete"}"#);
    let auto = ClassifierQuestionHandler::new(
        classifier,
        fb.clone() as Arc<dyn QuestionHandler>,
        Arc::new(StubResolver {
            provider,
            model: "x".into(),
        }),
        DecisionContext::with_cwd("/tmp"),
        emitter.clone() as Arc<dyn EventEmitter>,
    );
    let outcome = auto.ask(vec![one_question(), one_question()]).await;
    assert_eq!(outcome.source, ResolveSource::Manual);
    let failed_count = emitter.count_kind(|p| {
        matches!(
            p,
            AgentEventPayload::ClassifierFailed { reason, .. } if reason.contains("count mismatch")
        )
    });
    assert!(failed_count >= 1);
}

#[tokio::test]
async fn classifier_abstain_defers_to_manual() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(150),
        UserQuestionResponse::answered("manual-id", vec!["米饭类".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let provider = ScriptedProvider::returning(
        r#"{"answers":[[]],"reason":"subjective preference; deferring"}"#,
    );
    let auto = ClassifierQuestionHandler::new(
        classifier,
        fb.clone() as Arc<dyn QuestionHandler>,
        Arc::new(StubResolver {
            provider,
            model: "x".into(),
        }),
        DecisionContext::with_cwd("/tmp"),
        emitter.clone() as Arc<dyn EventEmitter>,
    );
    let outcome = auto.ask(vec![one_question()]).await;
    assert_eq!(outcome.source, ResolveSource::Manual);
    let abstain_count = emitter.count_kind(|p| {
        matches!(
            p,
            AgentEventPayload::ClassifierFailed { reason, .. } if reason.contains("abstain")
        )
    });
    assert!(
        abstain_count >= 1,
        "abstain reason should appear in ClassifierFailed"
    );
}
