use loopal_auto_mode::parse_question_response_for_test;

mod llm_path {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use async_trait::async_trait;
    use loopal_auto_mode::AutoClassifier;
    use loopal_error::LoopalError;
    use loopal_protocol::{Question, QuestionOption};
    use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};

    struct OkProvider {
        text: std::sync::Mutex<Option<String>>,
    }

    struct StreamErrProvider;

    struct ChunkErrProvider {
        text_before_err: std::sync::Mutex<Option<String>>,
    }

    struct S(VecDeque<Result<StreamChunk, LoopalError>>);
    impl futures::Stream for S {
        type Item = Result<StreamChunk, LoopalError>;
        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            std::task::Poll::Ready(self.0.pop_front())
        }
    }
    impl Unpin for S {}

    #[async_trait]
    impl Provider for OkProvider {
        fn name(&self) -> &str {
            "ok"
        }
        async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
            let t = self.text.lock().unwrap().take().unwrap();
            let chunks = VecDeque::from(vec![
                Ok(StreamChunk::Text { text: t }),
                Ok(StreamChunk::Done {
                    stop_reason: StopReason::EndTurn,
                }),
            ]);
            Ok(Box::pin(S(chunks)))
        }
    }

    #[async_trait]
    impl Provider for StreamErrProvider {
        fn name(&self) -> &str {
            "stream-err"
        }
        async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
            Err(LoopalError::Other("stream open failed".into()))
        }
    }

    #[async_trait]
    impl Provider for ChunkErrProvider {
        fn name(&self) -> &str {
            "chunk-err"
        }
        async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
            let pre = self
                .text_before_err
                .lock()
                .unwrap()
                .take()
                .unwrap_or_default();
            let chunks = VecDeque::from(vec![
                Ok(StreamChunk::Text { text: pre }),
                Err(LoopalError::Other("chunk read failed mid-stream".into())),
            ]);
            Ok(Box::pin(S(chunks)))
        }
    }

    fn one_question() -> Vec<Question> {
        vec![Question {
            question: "go?".into(),
            options: vec![QuestionOption {
                label: "yes".into(),
                description: "".into(),
            }],
            allow_multiple: false,
        }]
    }

    #[tokio::test]
    async fn success_returns_parsed_result_and_records_approval() {
        let classifier = AutoClassifier::new(String::new());
        let provider = Arc::new(OkProvider {
            text: std::sync::Mutex::new(Some(r#"{"answers": [["yes"]], "reason": "ok"}"#.into())),
        });
        let r = classifier
            .classify_question(&one_question(), "", "/tmp", &*provider, "model")
            .await;
        assert!(r.error.is_none(), "success path: error must be None");
        assert_eq!(r.answers, vec![vec!["yes".to_string()]]);
        assert_eq!(r.reason, "ok");
        assert!(
            !classifier.is_degraded(),
            "single success must not trip breaker"
        );
    }

    #[tokio::test]
    async fn stream_open_error_records_breaker_error() {
        let classifier = AutoClassifier::new(String::new());
        let provider = Arc::new(StreamErrProvider);
        // Trip breaker (3 consecutive errors)
        for _ in 0..3 {
            let r = classifier
                .classify_question(&one_question(), "", "/tmp", &*provider, "model")
                .await;
            assert!(r.error.is_some(), "stream error must populate error field");
            assert!(
                r.error.as_ref().unwrap().contains("stream"),
                "error message should mention stream"
            );
        }
        assert!(
            classifier.is_degraded(),
            "3 consecutive stream errors must trip the breaker (shared with permission path)"
        );
    }

    #[tokio::test]
    async fn chunk_error_mid_stream_records_error() {
        let classifier = AutoClassifier::new(String::new());
        let provider = Arc::new(ChunkErrProvider {
            text_before_err: std::sync::Mutex::new(Some("partial".into())),
        });
        let r = classifier
            .classify_question(&one_question(), "", "/tmp", &*provider, "model")
            .await;
        assert!(
            r.error.is_some(),
            "chunk error must populate the error field"
        );
        assert!(
            r.error.as_ref().unwrap().contains("chunk"),
            "error msg should identify chunk-level failure, got: {:?}",
            r.error
        );
    }

    #[tokio::test]
    async fn parse_failure_records_error_and_breaker_eventually_trips() {
        let classifier = AutoClassifier::new(String::new());
        // Make 3 calls with non-JSON response — parse failure each time
        for _ in 0..3 {
            let provider = Arc::new(OkProvider {
                text: std::sync::Mutex::new(Some("not json at all".into())),
            });
            let r = classifier
                .classify_question(&one_question(), "", "/tmp", &*provider, "model")
                .await;
            assert!(r.error.is_some(), "parse failure must populate error");
            assert!(r.answers.is_empty(), "no answers on parse failure");
        }
        assert!(
            classifier.is_degraded(),
            "3 parse failures should trip the breaker"
        );
    }

    #[tokio::test]
    async fn success_resets_consecutive_breaker_counter() {
        let classifier = AutoClassifier::new(String::new());
        // 2 errors, then 1 success → no trip
        for _ in 0..2 {
            let provider = Arc::new(StreamErrProvider);
            classifier
                .classify_question(&one_question(), "", "/tmp", &*provider, "model")
                .await;
        }
        assert!(!classifier.is_degraded(), "2 errors below threshold");
        let provider = Arc::new(OkProvider {
            text: std::sync::Mutex::new(Some(r#"{"answers": [["yes"]], "reason": "ok"}"#.into())),
        });
        let _ = classifier
            .classify_question(&one_question(), "", "/tmp", &*provider, "model")
            .await;
        // Now even a single error should not trip (counter reset)
        let provider = Arc::new(StreamErrProvider);
        classifier
            .classify_question(&one_question(), "", "/tmp", &*provider, "model")
            .await;
        assert!(
            !classifier.is_degraded(),
            "approval should have reset consecutive counter for @question key"
        );
    }
}

#[test]
fn parses_single_select_answers() {
    let raw = r#"{"answers": [["Option A"]], "reason": "user previously chose minimal style"}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(result.answers, vec![vec!["Option A".to_string()]]);
    assert_eq!(result.reason, "user previously chose minimal style");
}

#[test]
fn parses_multi_select_answers() {
    let raw = r#"{"answers": [["A", "B", "C"]], "reason": "all relevant"}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(
        result.answers,
        vec![vec!["A".to_string(), "B".to_string(), "C".to_string()]]
    );
}

#[test]
fn parses_multiple_questions() {
    let raw = r#"{"answers": [["Yes"], ["No"]], "reason": "diverging picks"}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(result.answers.len(), 2);
    assert_eq!(result.answers[0], vec!["Yes".to_string()]);
    assert_eq!(result.answers[1], vec!["No".to_string()]);
}

#[test]
fn strips_markdown_json_fence() {
    let raw = "```json\n{\"answers\": [[\"x\"]], \"reason\": \"y\"}\n```";
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(result.answers, vec![vec!["x".to_string()]]);
}

#[test]
fn strips_bare_markdown_fence() {
    let raw = "```\n{\"answers\": [[\"a\"]], \"reason\": \"r\"}\n```";
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(result.answers, vec![vec!["a".to_string()]]);
}

#[test]
fn returns_error_on_invalid_json() {
    let raw = "not json at all";
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_some(), "should report parse failure");
    assert!(result.answers.is_empty());
}

#[test]
fn returns_empty_answers_when_field_missing() {
    let raw = r#"{"reason": "no answers"}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert!(result.answers.is_empty());
    assert_eq!(result.reason, "no answers");
}

#[test]
fn ignores_non_string_labels() {
    let raw = r#"{"answers": [["valid", 42, null, "also-valid"]], "reason": "mixed"}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(
        result.answers[0],
        vec!["valid".to_string(), "also-valid".to_string()]
    );
}

#[test]
fn missing_reason_field_yields_empty_string() {
    let raw = r#"{"answers": [["x"]]}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(result.reason, "");
}

#[test]
fn handles_nested_inner_non_array_as_empty() {
    let raw = r#"{"answers": [null, ["b"]], "reason": "skip"}"#;
    let result = parse_question_response_for_test(raw);
    assert!(result.error.is_none());
    assert_eq!(result.answers.len(), 2);
    assert!(result.answers[0].is_empty());
    assert_eq!(result.answers[1], vec!["b".to_string()]);
}
