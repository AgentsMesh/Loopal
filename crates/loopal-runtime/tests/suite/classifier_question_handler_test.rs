use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use loopal_classifier::ClassifierEngine;
use loopal_protocol::{
    AgentEventPayload, Question, QuestionOption, ResolveSource, UserQuestionResponse,
};
use loopal_runtime::frontend::question_handler::{QuestionHandler, QuestionOutcome};
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::frontend::{ClassifierQuestionHandler, DecisionContext};

use super::classifier_question_handler_support::{
    DelayedFallback, FailingResolver, RecordingEmitter, ScriptedProvider, StubResolver,
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
async fn resolver_failure_skips_race_and_goes_pure_manual() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(0),
        UserQuestionResponse::answered("q-1", vec!["yes".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let auto = ClassifierQuestionHandler::new(
        classifier,
        fb.clone() as Arc<dyn QuestionHandler>,
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp"),
        emitter.clone() as Arc<dyn EventEmitter>,
    );
    let outcome = auto.ask(vec![one_question()]).await;
    assert_eq!(outcome.source, ResolveSource::Manual);
    assert_eq!(fb.call_count.load(Ordering::SeqCst), 1);
    // Pure manual path skips race → no ClassifierEngine* events should be emitted
    assert_eq!(
        emitter.count_kind(|p| matches!(
            p,
            AgentEventPayload::ClassifierProgress { .. }
                | AgentEventPayload::ClassifierFailed { .. }
                | AgentEventPayload::ClassifierCompleted { .. }
        )),
        0
    );
}

#[tokio::test]
async fn degraded_classifier_skips_race() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    classifier.force_degraded_for_test("@question");
    assert!(classifier.is_degraded());
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(0),
        UserQuestionResponse::answered("q-2", vec!["yes".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let provider = ScriptedProvider::returning(r#"{"answers":[["yes"]],"reason":""}"#);
    let auto = ClassifierQuestionHandler::new(
        classifier,
        fb.clone() as Arc<dyn QuestionHandler>,
        Arc::new(StubResolver {
            provider,
            model: "x".into(),
        }),
        DecisionContext::with_cwd("/tmp"),
        emitter as Arc<dyn EventEmitter>,
    );
    let outcome = auto.ask(vec![one_question()]).await;
    assert_eq!(outcome.source, ResolveSource::Manual);
    assert_eq!(fb.call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn classifier_wins_when_fallback_is_slow() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(300),
        UserQuestionResponse::answered("manual-id", vec!["no".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let provider =
        ScriptedProvider::returning(r#"{"answers":[["yes"]],"reason":"classifier picks yes"}"#);
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
    assert_eq!(outcome.source, ResolveSource::Classifier);
    assert!(outcome.reason.contains("classifier picks yes"));
    // Emitter should have Completed + Resolved{by:Auto}
    assert!(
        emitter.count_kind(|p| matches!(p, AgentEventPayload::ClassifierCompleted { .. })) >= 1
    );
    assert!(
        emitter.count_kind(|p| matches!(
            p,
            AgentEventPayload::UserQuestionResolved {
                by: ResolveSource::Classifier,
                ..
            }
        )) >= 1
    );
    // Fallback options were passed in with classifier_running=true
    if let Some(opts) = fb.last_options.lock().unwrap().as_ref() {
        assert!(opts.classifier_running);
    } else {
        panic!("fallback never received options");
    }
}

#[tokio::test]
async fn manual_wins_when_classifier_is_slow() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(0),
        UserQuestionResponse::answered("manual-id", vec!["yes".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let provider = ScriptedProvider::returning_after(
        r#"{"answers":[["no"]],"reason":"classifier picks no"}"#,
        Duration::from_millis(500),
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
    assert_eq!(
        emitter.count_kind(|p| matches!(
            p,
            AgentEventPayload::UserQuestionResolved {
                by: ResolveSource::Classifier,
                ..
            }
        )),
        0
    );
}

#[test]
fn outcome_constructors_set_source_correctly() {
    let manual = QuestionOutcome::manual(UserQuestionResponse::answered("a", vec!["x".into()]));
    assert_eq!(manual.source, ResolveSource::Manual);
    let cancelled = QuestionOutcome::cancelled("b", "boom");
    assert_eq!(cancelled.source, ResolveSource::Manual);
    let auto = QuestionOutcome::classifier(
        UserQuestionResponse::answered("c", vec!["y".into()]),
        "r".into(),
        42,
    );
    assert_eq!(auto.source, ResolveSource::Classifier);
    assert_eq!(auto.duration_ms, 42);
}
