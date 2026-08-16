use std::sync::Arc;
use std::time::Duration;

use loopal_backend::shell::SpawnedBackgroundData;
use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_tool_api::{Backend, EnvOverride, ExecOutcome, OutputTail};

use crate::log_file_test_support::unique_session_id;
use crate::log_guard_support::process_sanitizer;

fn assert_guarded(value: &str, secret: &str) {
    assert!(!value.contains(secret), "plaintext leaked in {value:?}");
    assert!(value.contains("<secret_ref:token>"));
}

#[tokio::test]
#[cfg(unix)]
async fn local_backend_routes_every_guarded_process_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let secret = "local-process-secret";
    let sanitizer = process_sanitizer(secret);
    let backend = LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        ResourceLimits::default(),
        unique_session_id(),
    );
    let env = EnvOverride::default();
    let command = "printf 'local-process-secret\\n'";

    let foreground = backend
        .exec_guarded(
            command,
            Duration::from_secs(5),
            &env,
            Some(sanitizer.clone()),
        )
        .await
        .unwrap();
    assert_guarded(&foreground.stdout, secret);
    assert_guarded(
        &std::fs::read_to_string(&foreground.log_path).unwrap(),
        secret,
    );

    let tail = Arc::new(OutputTail::new(20));
    let streaming = backend
        .exec_streaming_guarded(
            command,
            Duration::from_secs(5),
            &env,
            tail.clone(),
            Some(sanitizer.clone()),
        )
        .await
        .unwrap();
    let ExecOutcome::Completed(streaming) = streaming else {
        panic!("expected completed streaming command");
    };
    assert_guarded(&streaming.stdout, secret);
    assert_guarded(&tail.snapshot(), secret);

    let handle = backend
        .exec_background_guarded(command, &env, Some(sanitizer))
        .await
        .unwrap();
    let data = handle.0.downcast::<SpawnedBackgroundData>().unwrap();
    let SpawnedBackgroundData {
        mut spawned,
        log_path,
        capture_task,
        ..
    } = *data;
    assert!(spawned.wait().await.unwrap().success());
    loopal_backend::process_capture_task::join(capture_task)
        .await
        .unwrap();
    assert_guarded(&std::fs::read_to_string(log_path).unwrap(), secret);
}
