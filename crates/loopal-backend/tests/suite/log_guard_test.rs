use std::time::Duration;

use loopal_backend::shell::exec_command_guarded;
use loopal_tool_api::EnvOverride;

use crate::log_file_test_support::unique_session_id;
use crate::log_guard_support::process_sanitizer;

async fn guarded(command: &str, secret: &str) -> loopal_tool_api::ExecResult {
    exec_command_guarded(
        &std::env::temp_dir(),
        None,
        command,
        &EnvOverride::default(),
        Duration::from_secs(5),
        &unique_session_id(),
        Some(process_sanitizer(secret)),
    )
    .await
    .unwrap()
}

fn assert_all_guarded(result: &loopal_tool_api::ExecResult, secret: &str) {
    let log = std::fs::read_to_string(&result.log_path).unwrap();
    for value in [&result.stdout, &result.stderr, &log] {
        assert!(!value.contains(secret), "plaintext leaked in {value:?}");
        assert!(
            value.contains("<secret_ref:token>"),
            "placeholder missing in {value:?}"
        );
    }
}

#[tokio::test]
#[cfg(unix)]
async fn guarded_stdout_crosses_process_write_boundaries() {
    let secret = "split-secret";
    let result = guarded(
        "printf 'before-split-'; sleep 0.02; printf 'secret-after\\n'",
        secret,
    )
    .await;
    let log = std::fs::read_to_string(&result.log_path).unwrap();
    assert!(!result.stdout.contains(secret));
    assert!(!log.contains(secret));
    assert!(result.stdout.contains("<secret_ref:token>"));
    assert!(log.contains("<secret_ref:token>"));
}

#[tokio::test]
#[cfg(unix)]
async fn stderr_framing_precedes_guarding() {
    let secret = "[err] framed-secret";
    let result = guarded("printf 'framed-secret' >&2", secret).await;
    let log = std::fs::read_to_string(&result.log_path).unwrap();
    assert!(!log.contains(secret));
    assert!(log.contains("<secret_ref:token>"));
    assert!(!result.stderr.contains(secret));
}

#[tokio::test]
#[cfg(unix)]
async fn serialized_cross_stream_output_is_guarded() {
    let secret = "left[err] right";
    let result = guarded("printf 'left'; sleep 0.05; printf 'right' >&2", secret).await;
    let log = std::fs::read_to_string(&result.log_path).unwrap();
    assert!(!log.contains(secret));
    assert!(log.contains("<secret_ref:token>"));
}

#[tokio::test]
#[cfg(unix)]
async fn invalid_utf8_is_normalized_before_guarding() {
    let secret = "left�right";
    let result = guarded("printf 'left\\377right'", secret).await;
    let log = std::fs::read_to_string(&result.log_path).unwrap();
    assert!(!log.contains(secret));
    assert!(log.contains("<secret_ref:token>"));
}

#[tokio::test]
#[cfg(unix)]
async fn guarded_stdout_and_stderr_remain_seeded_to_one_call() {
    let secret = "same-secret";
    let result = guarded(
        "printf 'same-secret\\n'; printf 'same-secret\\n' >&2",
        secret,
    )
    .await;
    assert_all_guarded(&result, secret);
}
