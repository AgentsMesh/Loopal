// Integration test of outraced telemetry IO behavior via env override.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use loopal_classifier::ClassifierEngine;
use loopal_protocol::{Question, QuestionOption, ResolveSource, UserQuestionResponse};
use loopal_runtime::frontend::question_handler::QuestionHandler;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::frontend::{ClassifierQuestionHandler, DecisionContext};

use super::classifier_question_handler_support::{
    DelayedFallback, RecordingEmitter, ScriptedProvider, StubResolver,
};

struct EnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn one_question() -> Question {
    Question {
        question: "Pick".into(),
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
    }
}

fn read_jsonl(path: &PathBuf) -> Vec<serde_json::Value> {
    let s = fs::read_to_string(path).unwrap_or_default();
    s.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// Poll the telemetry file until `expected_lines` rows are present or we
// exceed `max_wait`. Replaces a fixed `sleep` so the test doesn't fail
// when CI is under load.
async fn wait_for_rows(path: &PathBuf, expected_lines: usize, max_wait: Duration) {
    let start = std::time::Instant::now();
    loop {
        if read_jsonl(path).len() >= expected_lines {
            return;
        }
        if start.elapsed() >= max_wait {
            panic!(
                "telemetry never reached {expected_lines} rows within {max_wait:?}; saw {:?}",
                read_jsonl(path)
            );
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

// Single combined test — sharing LOOPAL_TELEMETRY_DIR across two parallel
// tokio tests races on a process-global env var. Run two scenarios serially.
#[tokio::test]
async fn telemetry_full_fields_and_order_insensitive_agreement() {
    let prev = std::env::var("LOOPAL_TELEMETRY_DIR").ok();
    let _g = EnvGuard {
        key: "LOOPAL_TELEMETRY_DIR",
        prev,
    };
    let dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("LOOPAL_TELEMETRY_DIR", dir.path()) };
    let path = dir.path().join("classifier_outraced.jsonl");
    let _ = fs::remove_file(&path);

    // ── Scenario A: disagreement → agreement = false + all fields populated ──
    {
        let classifier = Arc::new(ClassifierEngine::new("".into()));
        let fb = Arc::new(DelayedFallback::new(
            Duration::from_millis(0),
            UserQuestionResponse::answered("manual-id", vec!["yes".into()]),
        ));
        let emitter = Arc::new(RecordingEmitter::new());
        let provider = ScriptedProvider::returning_after(
            r#"{"answers":[["no"]],"reason":"classifier disagrees"}"#,
            Duration::from_millis(200),
        );
        let auto = ClassifierQuestionHandler::new(
            classifier,
            fb as Arc<dyn QuestionHandler>,
            Arc::new(StubResolver {
                provider,
                model: "x".into(),
            }),
            DecisionContext::with_cwd("/tmp"),
            emitter as Arc<dyn EventEmitter>,
        );
        let outcome = auto.ask(vec![one_question()]).await;
        assert_eq!(outcome.source, ResolveSource::Manual);
    }
    wait_for_rows(&path, 1, Duration::from_secs(5)).await;

    // ── Scenario B: same labels, different order → agreement = true ──
    {
        let classifier = Arc::new(ClassifierEngine::new("".into()));
        let fb = Arc::new(DelayedFallback::new(
            Duration::from_millis(0),
            UserQuestionResponse::answered("manual-id", vec!["A".into(), "B".into()]),
        ));
        let emitter = Arc::new(RecordingEmitter::new());
        let provider = ScriptedProvider::returning_after(
            r#"{"answers":[["B"],["A"]],"reason":"order reversed"}"#,
            Duration::from_millis(200),
        );
        let auto = ClassifierQuestionHandler::new(
            classifier,
            fb as Arc<dyn QuestionHandler>,
            Arc::new(StubResolver {
                provider,
                model: "x".into(),
            }),
            DecisionContext::with_cwd("/tmp"),
            emitter as Arc<dyn EventEmitter>,
        );
        let outcome = auto.ask(vec![one_question(), one_question()]).await;
        assert_eq!(outcome.source, ResolveSource::Manual);
    }
    wait_for_rows(&path, 2, Duration::from_secs(5)).await;

    let rows = read_jsonl(&path);
    assert_eq!(
        rows.len(),
        2,
        "exactly two telemetry rows expected: {rows:#?}"
    );

    // Row 0 — full-field validation
    let r0 = &rows[0];
    assert!(r0["ts"].as_str().unwrap().contains('T'), "ts: {r0}");
    assert!(!r0["question_id"].as_str().unwrap().is_empty());
    assert_eq!(r0["manual_answers"], serde_json::json!(["yes"]));
    assert_eq!(r0["classifier_answers"], serde_json::json!(["no"]));
    assert!(r0["manual_duration_ms"].is_u64());
    assert!(r0["classifier_duration_ms"].is_u64());
    assert_eq!(r0["agreement"], false);

    // Row 1 — order-insensitive agreement
    let r1 = &rows[1];
    assert_eq!(r1["manual_answers"], serde_json::json!(["A", "B"]));
    assert_eq!(r1["classifier_answers"], serde_json::json!(["B", "A"]));
    assert_eq!(
        r1["agreement"], true,
        "multi-set equal answers must agree regardless of order"
    );
}
