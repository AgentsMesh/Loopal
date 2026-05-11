use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_auto_mode::AutoClassifier;
use loopal_error::LoopalError;
use loopal_protocol::{Question, QuestionOption, UserQuestionResponse};
use loopal_provider_api::{
    ChatParams, ChatStream, Provider, ProviderResolver, StopReason, StreamChunk, TaskType,
};
use loopal_runtime::frontend::question_handler::{QuestionHandler, QuestionOutcome};
use loopal_runtime::frontend::{AutoQuestionHandler, DecisionContext};

struct FailingResolver;

impl ProviderResolver for FailingResolver {
    fn resolve_for(&self, _task: TaskType) -> Result<(String, Arc<dyn Provider>), LoopalError> {
        Err(LoopalError::Other("resolver failure".into()))
    }
}

struct MockResolver {
    provider: Arc<dyn Provider>,
    model: String,
}

impl ProviderResolver for MockResolver {
    fn resolve_for(&self, _task: TaskType) -> Result<(String, Arc<dyn Provider>), LoopalError> {
        Ok((self.model.clone(), self.provider.clone()))
    }
}

struct MockProvider {
    response: std::sync::Mutex<Option<String>>,
}

impl MockProvider {
    fn returning(json: &str) -> Arc<Self> {
        Arc::new(Self {
            response: std::sync::Mutex::new(Some(json.to_string())),
        })
    }
}

struct MockStream(VecDeque<Result<StreamChunk, LoopalError>>);
impl futures::Stream for MockStream {
    type Item = Result<StreamChunk, LoopalError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.0.pop_front())
    }
}
impl Unpin for MockStream {}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let text = self.response.lock().unwrap().take().unwrap();
        let chunks = VecDeque::from(vec![
            Ok(StreamChunk::Text { text }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ]);
        Ok(Box::pin(MockStream(chunks)))
    }
}

struct RecordingFallback {
    call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl QuestionHandler for RecordingFallback {
    async fn ask(&self, _q: Vec<Question>) -> QuestionOutcome {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        QuestionOutcome::cancelled("fallback-id", "mock fallback")
    }
}

#[tokio::test]
async fn falls_back_when_resolver_fails() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let fb_count = Arc::new(AtomicUsize::new(0));
    let fb = RecordingFallback {
        call_count: fb_count.clone(),
    };
    let auto = AutoQuestionHandler::new(
        classifier,
        Box::new(fb),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp"),
    );
    let q = Question {
        question: "test?".into(),
        options: vec![],
        allow_multiple: false,
    };
    let outcome = auto.ask(vec![q]).await;
    assert!(matches!(
        outcome.response,
        UserQuestionResponse::Cancelled { .. }
    ));
    assert_eq!(fb_count.load(Ordering::SeqCst), 1);
    assert!(
        !outcome.reason.is_empty(),
        "fallback outcome should carry a reason"
    );
}

#[tokio::test]
async fn empty_questions_still_falls_back() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let fb_count = Arc::new(AtomicUsize::new(0));
    let fb = RecordingFallback {
        call_count: fb_count.clone(),
    };
    let auto = AutoQuestionHandler::new(
        classifier,
        Box::new(fb),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp"),
    );
    let _ = auto.ask(vec![]).await;
    assert_eq!(fb_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn degraded_classifier_skips_provider_and_falls_back() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    classifier.force_degraded_for_test("@question");
    assert!(
        classifier.is_degraded(),
        "precondition: classifier degraded"
    );

    let fb_count = Arc::new(AtomicUsize::new(0));
    let fb = RecordingFallback {
        call_count: fb_count.clone(),
    };
    let auto = AutoQuestionHandler::new(
        classifier,
        Box::new(fb),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp"),
    );
    let q = Question {
        question: "test?".into(),
        options: vec![],
        allow_multiple: false,
    };
    let outcome = auto.ask(vec![q]).await;
    assert_eq!(
        fb_count.load(Ordering::SeqCst),
        1,
        "fallback must be invoked exactly once"
    );
    assert!(
        outcome.reason.contains("classifier degraded"),
        "reason should record the trigger (classifier degraded), got: {}",
        outcome.reason
    );
}

#[test]
fn cancelled_constructor_sets_response_and_reason() {
    let outcome = QuestionOutcome::cancelled("q-7", "user disconnected");
    match outcome.response {
        UserQuestionResponse::Cancelled { question_id } => {
            assert_eq!(question_id, "q-7");
        }
        _ => panic!("expected Cancelled, got {:?}", outcome.response),
    }
    assert_eq!(outcome.reason, "user disconnected");
    assert_eq!(outcome.duration_ms, 0);
}

#[test]
fn manual_constructor_keeps_supplied_response() {
    let original = UserQuestionResponse::answered("q-3", vec!["yes".into()]);
    let outcome = QuestionOutcome::manual(original.clone());
    match (&outcome.response, &original) {
        (
            UserQuestionResponse::Answered { question_id: a, .. },
            UserQuestionResponse::Answered { question_id: b, .. },
        ) => assert_eq!(a, b),
        _ => panic!("response mismatch"),
    }
    assert!(outcome.reason.is_empty());
    assert_eq!(outcome.duration_ms, 0);
}

#[tokio::test]
async fn happy_path_single_select_returns_answered_without_fallback() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let provider = MockProvider::returning(
        r#"{"answers": [["yes"]], "reason": "user previously approved this approach"}"#,
    );
    let resolver = Arc::new(MockResolver {
        provider,
        model: "claude-haiku".into(),
    });
    let fb_count = Arc::new(AtomicUsize::new(0));
    let fb = RecordingFallback {
        call_count: fb_count.clone(),
    };
    let auto = AutoQuestionHandler::new(
        classifier,
        Box::new(fb),
        resolver,
        DecisionContext::with_cwd("/tmp"),
    );
    let q = Question {
        question: "Proceed?".into(),
        options: vec![
            QuestionOption {
                label: "yes".into(),
                description: "go ahead".into(),
            },
            QuestionOption {
                label: "no".into(),
                description: "stop".into(),
            },
        ],
        allow_multiple: false,
    };
    let outcome = auto.ask(vec![q]).await;
    assert_eq!(
        fb_count.load(Ordering::SeqCst),
        0,
        "fallback must NOT be called on classifier success"
    );
    match outcome.response {
        UserQuestionResponse::Answered { answers, .. } => {
            assert_eq!(answers, vec!["yes".to_string()]);
        }
        other => panic!("expected Answered, got {other:?}"),
    }
    assert!(
        outcome.reason.contains("previously approved"),
        "reason must propagate classifier reason, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn happy_path_multi_select_joins_labels_with_comma() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let provider = MockProvider::returning(
        r#"{"answers": [["A", "B", "C"]], "reason": "all required for the task"}"#,
    );
    let resolver = Arc::new(MockResolver {
        provider,
        model: "claude-haiku".into(),
    });
    let fb = RecordingFallback {
        call_count: Arc::new(AtomicUsize::new(0)),
    };
    let auto = AutoQuestionHandler::new(
        classifier,
        Box::new(fb),
        resolver,
        DecisionContext::with_cwd("/tmp"),
    );
    let q = Question {
        question: "Select all that apply".into(),
        options: vec![
            QuestionOption {
                label: "A".into(),
                description: "".into(),
            },
            QuestionOption {
                label: "B".into(),
                description: "".into(),
            },
            QuestionOption {
                label: "C".into(),
                description: "".into(),
            },
        ],
        allow_multiple: true,
    };
    let outcome = auto.ask(vec![q]).await;
    match outcome.response {
        UserQuestionResponse::Answered { answers, .. } => {
            // For multi-select, AutoQuestionHandler joins inner labels with ", "
            assert_eq!(answers, vec!["A, B, C".to_string()]);
        }
        other => panic!("expected Answered with comma-joined labels, got {other:?}"),
    }
}

#[tokio::test]
async fn answer_count_mismatch_falls_back() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    // Two questions but LLM returns only one answer
    let provider = MockProvider::returning(r#"{"answers": [["yes"]], "reason": "incomplete"}"#);
    let resolver = Arc::new(MockResolver {
        provider,
        model: "claude-haiku".into(),
    });
    let fb_count = Arc::new(AtomicUsize::new(0));
    let fb = RecordingFallback {
        call_count: fb_count.clone(),
    };
    let auto = AutoQuestionHandler::new(
        classifier,
        Box::new(fb),
        resolver,
        DecisionContext::with_cwd("/tmp"),
    );
    let q = || Question {
        question: "Pick one".into(),
        options: vec![QuestionOption {
            label: "a".into(),
            description: "".into(),
        }],
        allow_multiple: false,
    };
    let outcome = auto.ask(vec![q(), q()]).await;
    assert_eq!(
        fb_count.load(Ordering::SeqCst),
        1,
        "answer count mismatch must trigger fallback"
    );
    assert!(
        outcome.reason.contains("answer count mismatch"),
        "reason must record the mismatch trigger, got: {}",
        outcome.reason
    );
}
