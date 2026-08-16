#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::{OutputTail, ProcessOutputSanitizer, ProcessOutputStream};

use super::{capture_failed, exec_command_streaming_guarded, wait_failed};
use crate::shell_spawn::CapturePolicy;

struct PanicSanitizer;

impl ProcessOutputSanitizer for PanicSanitizer {
    fn stream(&self) -> Box<dyn ProcessOutputStream> {
        panic!("injected sanitizer panic")
    }

    fn guard_text(&self, text: &str) -> String {
        text.to_string()
    }
}

#[test]
fn streaming_failures_have_stable_error_text() {
    let wait_error = std::io::Error::other("injected wait failure");
    assert_eq!(
        wait_failed(&wait_error).to_string(),
        "exec failed: wait failed: injected wait failure"
    );
    assert_eq!(
        capture_failed().to_string(),
        "exec failed: process output capture failed"
    );
}

#[tokio::test]
async fn capture_failure_cleans_up_streaming_process() {
    let sanitizer: Arc<dyn ProcessOutputSanitizer> = Arc::new(PanicSanitizer);
    let session = format!("stream-capture-failure-{}", uuid::Uuid::new_v4().simple());
    let result = exec_command_streaming_guarded(
        &std::env::temp_dir(),
        None,
        "sleep 30",
        &loopal_tool_api::EnvOverride::default(),
        Duration::from_secs(10),
        Arc::new(OutputTail::new(4)),
        CapturePolicy::new(&session, Some(sanitizer)),
    )
    .await;
    let Err(error) = result else {
        panic!("expected capture failure");
    };

    assert!(
        error
            .to_string()
            .contains("process output capture task failed")
    );
}
