use std::sync::Arc;

use serde_json::json;

use crate::bash_secret_support::{CANARY, field, run, test_context};

#[cfg(unix)]
#[tokio::test]
async fn background_bash_redacts_before_process_result_and_log_persistence() {
    let session_id = format!("bg-secret-{}", std::process::id());
    let (kernel, ctx) = test_context(&session_id);

    let started = run(
        &kernel,
        &ctx,
        "background-secret",
        "Bash",
        json!({
            "command": "printf '%s\\n' \"$TOKEN\"; printf '%s\\n' \"$TOKEN\" >&2",
            "env": {"TOKEN": "<secret_ref:token>"},
            "run_in_background": true
        }),
    )
    .await;
    let process_id = field(&started.content, "process_id: ");
    let log_path = std::path::PathBuf::from(field(&started.content, "Full log: "));

    let output = run(
        &kernel,
        &ctx,
        "background-output",
        "BashProcess",
        json!({"process_id": process_id, "block": true, "timeout": 5}),
    )
    .await;
    assert!(!output.content.contains(CANARY));
    assert!(output.content.contains("<secret_ref:token>"));

    let log = std::fs::read_to_string(&log_path).unwrap();
    assert!(!log.contains(CANARY));
    assert!(log.contains("<secret_ref:token>"));
    assert!(log.contains("[err] <secret_ref:token>"));

    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&log_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(log_path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_conversion_keeps_redacting_after_the_bash_call_returns() {
    let session_id = format!("bg-timeout-secret-{}", std::process::id());
    let (kernel, mut ctx) = test_context(&session_id);
    ctx.output_tail = Some(Arc::new(loopal_tool_api::OutputTail::new(10)));

    let started = run(
        &kernel,
        &ctx,
        "timeout-secret",
        "Bash",
        json!({
            "command": "printf '%s\\n' \"$TOKEN\"; sleep 2; printf '%s\\n' \"$TOKEN\"",
            "env": {"TOKEN": "<secret_ref:token>"},
            "timeout": 1
        }),
    )
    .await;
    assert!(!started.content.contains(CANARY));
    assert!(started.content.contains("<secret_ref:token>"));
    assert!(started.content.contains("Use BashProcess with process_id"));
    let process_id = field(&started.content, "process_id: ");
    let log_path = std::path::PathBuf::from(field(&started.content, "Full log: "));

    let output = run(
        &kernel,
        &ctx,
        "timeout-output",
        "BashProcess",
        json!({"process_id": process_id, "block": true, "timeout": 5}),
    )
    .await;
    assert!(!output.content.contains(CANARY));
    assert_eq!(output.content.matches("<secret_ref:token>").count(), 2);

    let log = std::fs::read_to_string(log_path).unwrap();
    assert!(!log.contains(CANARY));
    assert_eq!(log.matches("<secret_ref:token>").count(), 2);
}
