/// Tests for snapshot_running() — protocol-level snapshot generation.
use loopal_protocol::BgTaskStatus;
use loopal_tool_background::{self, TaskStatus};

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn setup() {
    loopal_tool_background::clear_store();
}

#[test]
fn empty_store_returns_empty_vec() {
    let _g = LOCK.lock().unwrap();
    setup();
    let snaps = loopal_tool_background::snapshot_running();
    assert!(snaps.is_empty());
}

#[test]
fn returns_only_running_tasks() {
    let _g = LOCK.lock().unwrap();
    setup();
    loopal_tool_background::register_proxy("bg_1".into(), "running task".into());
    let handle = loopal_tool_background::register_proxy("bg_2".into(), "done task".into());
    handle.complete("output".into(), true);

    let snaps = loopal_tool_background::snapshot_running();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, "bg_1");
    assert_eq!(snaps[0].status, BgTaskStatus::Running);
}

#[test]
fn sorted_by_id() {
    let _g = LOCK.lock().unwrap();
    setup();
    loopal_tool_background::register_proxy("bg_c".into(), "c".into());
    loopal_tool_background::register_proxy("bg_a".into(), "a".into());
    loopal_tool_background::register_proxy("bg_b".into(), "b".into());

    let snaps = loopal_tool_background::snapshot_running();
    let ids: Vec<&str> = snaps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["bg_a", "bg_b", "bg_c"]);
}

#[test]
fn excludes_failed_tasks() {
    let _g = LOCK.lock().unwrap();
    setup();
    loopal_tool_background::register_proxy("bg_ok".into(), "ok".into());
    let handle = loopal_tool_background::register_proxy("bg_fail".into(), "fail".into());
    handle.complete("err".into(), false);

    let snaps = loopal_tool_background::snapshot_running();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, "bg_ok");
}

#[test]
fn snapshot_carries_description() {
    let _g = LOCK.lock().unwrap();
    setup();
    loopal_tool_background::register_proxy("bg_x".into(), "compiling project".into());

    let snaps = loopal_tool_background::snapshot_running();
    assert_eq!(snaps[0].description, "compiling project");
}
