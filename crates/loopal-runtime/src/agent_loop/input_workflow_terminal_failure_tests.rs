use loopal_protocol::WorkflowTerminalDisposition;

use super::tests::{apply, notification, runner, turns_file};

#[tokio::test]
async fn unreadable_persisted_delivery_is_retryable() {
    let temp = tempfile::tempdir().unwrap();
    let mut runner = runner(temp.path(), "session-unreadable");
    std::fs::write(turns_file(temp.path(), "session-unreadable"), b"not-json\n").unwrap();

    let (execute, disposition) = apply(&mut runner, notification("session-unreadable")).await;
    assert!(!execute);
    assert!(matches!(
        disposition,
        WorkflowTerminalDisposition::Retryable { reason }
            if reason.contains("failed to inspect persisted turns")
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn durable_turn_write_failure_is_retryable() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let mut runner = runner(temp.path(), "session-write-failure");
    let file = turns_file(temp.path(), "session-write-failure");
    std::fs::write(&file, []).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o400)).unwrap();

    let (execute, disposition) = apply(&mut runner, notification("session-write-failure")).await;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!execute);
    assert_eq!(
        disposition,
        WorkflowTerminalDisposition::Retryable {
            reason: "failed to durably persist workflow result turn".into(),
        }
    );
}
