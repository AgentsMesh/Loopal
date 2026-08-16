use std::path::PathBuf;
use std::sync::Arc;

use loopal_tool_api::ProcessOutputSanitizer;

use super::support::{TestReader, TestSink};
use crate::process_capture::spawn_with_sink;
use crate::process_capture_state::ProcessCaptureState;
use crate::process_capture_task;

async fn run_failure(
    reader: crate::process_capture::CaptureReader,
    sink: Arc<TestSink>,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
) -> (String, Arc<ProcessCaptureState>) {
    let state = ProcessCaptureState::new(PathBuf::from("/private/test.log"), sanitizer.clone());
    let task = spawn_with_sink(Some(reader), None, sink, state.clone(), None, sanitizer);
    let error = process_capture_task::join(task)
        .await
        .unwrap_err()
        .to_string();
    (error, state)
}

#[tokio::test]
async fn pipe_read_failure_is_typed_and_published() {
    let (error, state) = run_failure(TestReader::failing(), TestSink::new(false, None), None).await;
    assert!(error.contains("pipe read failed"));
    assert!(state.capture_failed());
    assert!(state.snapshot().stdout.is_empty());
}

#[tokio::test]
async fn log_write_failure_does_not_publish_preview() {
    let sink = TestSink::new(true, None);
    let (error, state) =
        run_failure(TestReader::chunks([b"uncommitted\n".to_vec()]), sink, None).await;
    assert!(error.contains("log write failed"));
    assert!(state.capture_failed());
    assert!(state.snapshot().stdout.is_empty());
}

#[tokio::test]
async fn chunk_flush_failure_does_not_publish_preview() {
    let sink = TestSink::new(false, Some(1));
    let (error, state) =
        run_failure(TestReader::chunks([b"uncommitted\n".to_vec()]), sink, None).await;
    assert!(error.contains("log flush failed"));
    assert!(state.capture_failed());
    assert!(state.snapshot().stdout.is_empty());
}

#[tokio::test]
async fn final_flush_failure_fails_empty_capture() {
    let (error, state) =
        run_failure(TestReader::chunks([]), TestSink::new(false, Some(1)), None).await;
    assert!(error.contains("final log flush failed"));
    assert!(state.capture_failed());
}

#[tokio::test]
async fn sanitizer_constructor_panic_becomes_capture_failure() {
    struct PanicSanitizer;
    impl ProcessOutputSanitizer for PanicSanitizer {
        fn stream(&self) -> Box<dyn loopal_tool_api::ProcessOutputStream> {
            panic!("injected sanitizer panic")
        }
        fn guard_text(&self, text: &str) -> String {
            text.to_string()
        }
    }
    let sanitizer: Arc<dyn ProcessOutputSanitizer> = Arc::new(PanicSanitizer);
    let (error, state) = run_failure(
        TestReader::chunks([]),
        TestSink::new(false, None),
        Some(sanitizer),
    )
    .await;
    assert!(error.contains("task failed"));
    assert!(state.capture_failed());
}
