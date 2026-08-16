use loopal_secret_runtime::{JsonlAuditSink, RuntimeOp, default_telemetry_dir};
use tempfile::tempdir;

#[test]
fn default_telemetry_path_is_under_loopal_home() {
    if let Some(path) = default_telemetry_dir() {
        assert!(path.ends_with(".loopal/telemetry"));
    }
}

#[test]
fn try_new_creates_an_empty_durable_log() {
    let dir = tempdir().unwrap();
    let sink = JsonlAuditSink::try_new(dir.path().join("nested")).unwrap();
    let path = dir.path().join("nested/secret_access.jsonl");
    assert_eq!(std::fs::read(&path).unwrap(), b"");
    sink.record_runtime(RuntimeOp::Resolved, &[], &Default::default())
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"");
}

#[test]
fn try_new_reports_directory_creation_failure() {
    let dir = tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let error = match JsonlAuditSink::try_new(blocker) {
        Ok(_) => panic!("expected creation failure"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("directory creation failed"));
}

#[test]
fn try_new_reports_open_failure() {
    let dir = tempdir().unwrap();
    std::fs::create_dir(dir.path().join("secret_access.jsonl")).unwrap();
    let error = match JsonlAuditSink::try_new(dir.path().to_path_buf()) {
        Ok(_) => panic!("expected open failure"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("file open failed"));
}
