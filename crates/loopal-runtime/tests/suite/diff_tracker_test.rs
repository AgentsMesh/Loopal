use loopal_message::ContentBlock;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, InterruptSignal};
use loopal_runtime::UnifiedFrontend;
use loopal_runtime::agent_loop::cancel::TurnCancel;
use loopal_runtime::agent_loop::diff_tracker::DiffTracker;
use loopal_runtime::agent_loop::turn_context::TurnContext;
use loopal_runtime::agent_loop::turn_observer::TurnObserver;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

fn make_ctx() -> TurnContext {
    let cancel = TurnCancel::new(InterruptSignal::new(), Arc::new(tokio::sync::Notify::new()));
    TurnContext::new(0, cancel)
}

fn make_tracker() -> DiffTracker {
    let (event_tx, _rx) = mpsc::channel::<AgentEvent>(16);
    let (_mbox_tx, mbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mbox_rx,
        ctrl_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    DiffTracker::new(frontend)
}

fn write_tool(id: &str, path: &str) -> (String, String, serde_json::Value) {
    (
        id.into(),
        "Write".into(),
        json!({"file_path": path, "content": "x"}),
    )
}

fn edit_tool(id: &str, path: &str) -> (String, String, serde_json::Value) {
    (
        id.into(),
        "Edit".into(),
        json!({"file_path": path, "old_string": "a", "new_string": "b"}),
    )
}

fn read_tool(id: &str, path: &str) -> (String, String, serde_json::Value) {
    (id.into(), "Read".into(), json!({"file_path": path}))
}

fn ok_result(id: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.into(),
        content: "ok".into(),
        is_error: false,
    }
}

fn err_result(id: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.into(),
        content: "err".into(),
        is_error: true,
    }
}

#[test]
fn tracks_write_tool_file_path() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let tools = [write_tool("t1", "/tmp/foo.rs")];
    let results = [ok_result("t1")];
    tracker.on_after_tools(&mut ctx, &tools, &results);
    assert!(ctx.modified_files.contains("/tmp/foo.rs"));
}

#[test]
fn tracks_edit_tool_file_path() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let tools = [edit_tool("t1", "/tmp/bar.rs")];
    let results = [ok_result("t1")];
    tracker.on_after_tools(&mut ctx, &tools, &results);
    assert!(ctx.modified_files.contains("/tmp/bar.rs"));
}

#[test]
fn ignores_read_tool() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let tools = [read_tool("t1", "/tmp/baz.rs")];
    let results = [ok_result("t1")];
    tracker.on_after_tools(&mut ctx, &tools, &results);
    assert!(ctx.modified_files.is_empty());
}

#[test]
fn ignores_failed_write_tool() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let tools = [write_tool("t1", "/tmp/fail.rs")];
    let results = [err_result("t1")];
    tracker.on_after_tools(&mut ctx, &tools, &results);
    assert!(
        ctx.modified_files.is_empty(),
        "failed writes should not be tracked"
    );
}

#[test]
fn deduplicates_same_file() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let tools = [write_tool("t1", "/tmp/x.rs"), edit_tool("t2", "/tmp/x.rs")];
    let results = [ok_result("t1"), ok_result("t2")];
    tracker.on_after_tools(&mut ctx, &tools, &results);
    assert_eq!(ctx.modified_files.len(), 1);
}

#[test]
fn tracks_multi_edit_array() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let multi_edit = (
        "t1".into(),
        "MultiEdit".into(),
        json!({
            "edits": [
                {"file_path": "/tmp/a.rs", "old_string": "x", "new_string": "y"},
                {"file_path": "/tmp/b.rs", "old_string": "x", "new_string": "y"},
            ]
        }),
    );
    let results = [ok_result("t1")];
    tracker.on_after_tools(&mut ctx, &[multi_edit], &results);
    assert!(ctx.modified_files.contains("/tmp/a.rs"));
    assert!(ctx.modified_files.contains("/tmp/b.rs"));
}

#[test]
fn tracks_notebook_edit() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let nb = (
        "t1".into(),
        "NotebookEdit".into(),
        json!({
            "notebook_path": "/tmp/nb.ipynb",
            "new_source": "x"
        }),
    );
    let results = [ok_result("t1")];
    tracker.on_after_tools(&mut ctx, &[nb], &results);
    assert!(ctx.modified_files.contains("/tmp/nb.ipynb"));
}

#[test]
fn on_turn_end_skips_when_no_files() {
    let mut tracker = make_tracker();
    let ctx = make_ctx();
    tracker.on_turn_end(&ctx);
}

#[test]
fn tracks_apply_patch() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let patch = (
        "t1".into(),
        "ApplyPatch".into(),
        json!({"file_path": "/tmp/p.rs"}),
    );
    let results = [ok_result("t1")];
    tracker.on_after_tools(&mut ctx, &[patch], &results);
    assert!(ctx.modified_files.contains("/tmp/p.rs"));
}

#[test]
fn mixed_success_and_failure() {
    let mut tracker = make_tracker();
    let mut ctx = make_ctx();
    let tools = [
        write_tool("t1", "/tmp/ok.rs"),
        write_tool("t2", "/tmp/fail.rs"),
        edit_tool("t3", "/tmp/also_ok.rs"),
    ];
    let results = [ok_result("t1"), err_result("t2"), ok_result("t3")];
    tracker.on_after_tools(&mut ctx, &tools, &results);
    assert!(ctx.modified_files.contains("/tmp/ok.rs"));
    assert!(!ctx.modified_files.contains("/tmp/fail.rs"));
    assert!(ctx.modified_files.contains("/tmp/also_ok.rs"));
}
