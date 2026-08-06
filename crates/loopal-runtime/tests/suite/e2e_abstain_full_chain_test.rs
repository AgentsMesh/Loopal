// E2E: abstain path across crates.
// Classifier returns `[[]]` → ClassifierQuestionHandler emits ClassifierFailed
// → fall back to manual → view-state mutator flips PendingQuestion.classifier_status
// to Failed{reason contains "abstain"} → after manual answers, conversation has
// the manual outcome (NOT the classifier's empty answer).

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use loopal_classifier::ClassifierEngine;
use loopal_protocol::{
    AgentEventPayload, Question, QuestionOption, ResolveSource, UserQuestionResponse,
};
use loopal_runtime::frontend::question_handler::QuestionHandler;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::frontend::{ClassifierQuestionHandler, DecisionContext};
use loopal_view_state::ViewStateReducer;
use loopal_view_state::conversation::ClassifierStatus;

use super::classifier_question_handler_support::{
    DelayedFallback, RecordingEmitter, ScriptedProvider, StubResolver,
};

fn make_question() -> Question {
    Question {
        question: "想吃什么？".into(),
        options: vec![
            QuestionOption {
                label: "米饭类".into(),
                description: "".into(),
            },
            QuestionOption {
                label: "面食".into(),
                description: "".into(),
            },
        ],
        allow_multiple: false,
        header: None,
    }
}

#[tokio::test]
async fn abstain_emits_failed_and_view_state_mirrors_it() {
    // Wire: classifier returns [[]] (abstain) — see question_prompt.rs spec.
    let provider = ScriptedProvider::returning(
        r#"{"answers":[[]],"reason":"subjective preference; deferring"}"#,
    );
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb = Arc::new(DelayedFallback::new(
        Duration::from_millis(120),
        UserQuestionResponse::answered("manual-id", vec!["米饭类".into()]),
    ));
    let emitter = Arc::new(RecordingEmitter::new());
    let handler = ClassifierQuestionHandler::new(
        classifier,
        fb.clone() as Arc<dyn QuestionHandler>,
        Arc::new(StubResolver {
            provider,
            model: "x".into(),
        }),
        DecisionContext::with_cwd("/tmp"),
        emitter.clone() as Arc<dyn EventEmitter>,
    );
    let outcome = handler.ask(vec![make_question()]).await;

    // Outcome routed through manual fallback → user's actual answer reaches LLM
    assert_eq!(outcome.source, ResolveSource::Manual);
    assert_eq!(fb.call_count.load(Ordering::SeqCst), 1);
    let answers = match &outcome.response {
        UserQuestionResponse::Answered { answers, .. } => answers.clone(),
        _ => panic!("expected Answered, got {:?}", outcome.response),
    };
    assert_eq!(answers, vec!["米饭类".to_string()]);

    // Wire event: a ClassifierFailed with abstain reason was broadcast
    let abstain_events: Vec<_> = emitter
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|p| match p {
            AgentEventPayload::ClassifierFailed { id, reason } => {
                Some((id.clone(), reason.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        abstain_events.len(),
        1,
        "exactly one ClassifierFailed expected"
    );
    let (qid, reason) = &abstain_events[0];
    assert!(
        reason.contains("abstain"),
        "reason should mention abstain: {reason}"
    );

    // Replay the wire events into a fresh view-state — UI status flips to Failed
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::UserQuestionRequest {
        id: qid.clone(),
        logical_id: qid.clone(),
        questions: vec![make_question()],
        classifier_running: true,
    });
    r.apply(AgentEventPayload::ClassifierFailed {
        id: qid.clone(),
        reason: reason.clone(),
    });
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .expect("pending_question present");
    match &pq.classifier_status {
        ClassifierStatus::Failed { reason: r } => {
            assert!(
                r.contains("abstain"),
                "status reason should mention abstain: {r}"
            );
        }
        s => panic!("expected Failed, got {s:?}"),
    }
}
