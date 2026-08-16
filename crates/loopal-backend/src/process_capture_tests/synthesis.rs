use std::path::PathBuf;
use std::sync::Arc;

use super::support::{TestReader, TestSink, sanitizer};
use crate::process_capture::spawn_with_sink;
use crate::process_capture_state::{ProcessCaptureState, ProcessCompletion};
use crate::process_capture_task;

async fn run(
    stdout: Vec<Vec<u8>>,
    stderr: Vec<Vec<u8>>,
    plaintext: &str,
) -> (Arc<ProcessCaptureState>, String) {
    run_readers(
        Some(TestReader::chunks(stdout)),
        Some(TestReader::chunks(stderr)),
        plaintext,
    )
    .await
}

async fn run_readers(
    stdout: Option<crate::process_capture::CaptureReader>,
    stderr: Option<crate::process_capture::CaptureReader>,
    plaintext: &str,
) -> (Arc<ProcessCaptureState>, String) {
    let guard = sanitizer(plaintext);
    let state = ProcessCaptureState::new(PathBuf::from("/private/test.log"), Some(guard.clone()));
    let sink = TestSink::new(false, None);
    let task = spawn_with_sink(
        stdout,
        stderr,
        sink.clone(),
        state.clone(),
        None,
        Some(guard),
    );
    process_capture_task::join(task).await.unwrap();
    (state, sink.text())
}

fn assert_guarded(value: &str, plaintext: &str) {
    assert!(!value.contains(plaintext), "plaintext leaked in {value:?}");
    assert!(
        value.contains("<secret_ref:token>"),
        "placeholder missing in {value:?}"
    );
}

#[tokio::test]
async fn stderr_prefix_is_framed_before_stream_guard() {
    let secret = "[err] framed-secret";
    let (_, log) = run(vec![], vec![b"framed-secret".to_vec()], secret).await;
    assert_guarded(&log, secret);
}

#[tokio::test]
async fn serialized_stream_guard_catches_cross_source_secret() {
    let secret = "left[err] right";
    let (_, log) = run_readers(
        Some(TestReader::chunks([b"left".to_vec()])),
        Some(TestReader::delayed([b"right".to_vec()])),
        secret,
    )
    .await;
    assert_guarded(&log, secret);
}

#[tokio::test]
async fn utf8_normalization_happens_before_guard() {
    let secret = "left�right";
    let (_, log) = run(vec![b"left\xffright".to_vec()], vec![], secret).await;
    assert_guarded(&log, secret);
}

#[tokio::test]
async fn stderr_label_composite_is_guarded_as_a_whole() {
    let secret = "[stderr]\nlabel-secret";
    let (state, _) = run(vec![], vec![b"label-secret".to_vec()], secret).await;
    assert_guarded(&state.render_preview(), secret);
}

#[tokio::test]
async fn truncation_suffix_composite_is_guarded_as_a_whole() {
    let secret = "needle [... line truncated ...]";
    let mut line = vec![b'x'; 64 * 1024 - "needle".len()];
    line.extend_from_slice(b"needle!");
    let (state, _) = run(vec![line], vec![], secret).await;
    assert_guarded(&state.render_preview(), secret);
}

#[tokio::test]
async fn elision_marker_composite_is_guarded_as_a_whole() {
    let secret = "HEAD_END\n[... 1 lines elided ...]\nTAIL_START";
    let mut output = String::new();
    for index in 1..=51 {
        let line = match index {
            25 => "HEAD_END".to_string(),
            27 => "TAIL_START".to_string(),
            _ => format!("line-{index}"),
        };
        output.push_str(&line);
        output.push('\n');
    }
    let (state, _) = run(vec![output.into_bytes()], vec![], secret).await;
    assert_guarded(&state.render_preview(), secret);
}

#[tokio::test]
async fn terminal_status_composite_is_guarded_as_a_whole() {
    let secret = "[stdout]\nstatus-secret\n\n[full log: /private/test.log]\n[Completed, exit 0]";
    let (state, _) = run(vec![b"status-secret".to_vec()], vec![], secret).await;
    state.finalize(ProcessCompletion::Completed, Some(0));
    assert_guarded(&state.render_output(false), secret);
}

#[test]
fn terminal_status_formats_every_completion_variant() {
    let cases = [
        (ProcessCompletion::Completed, None, "[Status: Completed]"),
        (ProcessCompletion::Failed, Some(7), "[Failed, exit 7]"),
        (ProcessCompletion::Failed, None, "[Status: Failed]"),
        (ProcessCompletion::Killed, Some(9), "[Killed, exit 9]"),
        (ProcessCompletion::Killed, None, "[Status: Killed]"),
    ];

    for (completion, exit_code, expected) in cases {
        assert_eq!(
            crate::process_capture_render::status_text(completion, exit_code),
            expected
        );
    }
}
