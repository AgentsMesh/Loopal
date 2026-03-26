//! E2E worktree tests: create, cwd-switch, exit-keep, exit-remove.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::{GitFixture, HarnessBuilder, assertions, chunks};
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

fn wrap_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        Vec::<CommandEntry>::new(),
        inner.fixture.path().to_path_buf(),
    );
    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

fn extract_result<'a>(evts: &'a [AgentEventPayload], tool: &str) -> Vec<&'a str> {
    evts.iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == tool => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn test_worktree_create() {
    let git = GitFixture::new();
    let calls = vec![
        chunks::tool_turn(
            "tc-wt",
            "EnterWorktree",
            serde_json::json!({"name": "test-wt-create"}),
        ),
        chunks::text_turn("Worktree created."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .cwd(git.path().to_path_buf())
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "EnterWorktree");
    assertions::assert_has_tool_result(&evts, "EnterWorktree", false);

    let wt_dir = git.path().join(".loopal/worktrees/test-wt-create");
    assert!(
        wt_dir.exists(),
        "worktree dir should exist at {}",
        wt_dir.display()
    );

    let results = extract_result(&evts, "EnterWorktree");
    assert!(
        results.iter().any(|r| r.contains("test-wt-create")),
        "result should mention worktree name, got: {results:?}"
    );
}

#[tokio::test]
async fn test_worktree_cwd_switch() {
    let git = GitFixture::new();
    let calls = vec![
        chunks::tool_turn(
            "tc-enter",
            "EnterWorktree",
            serde_json::json!({"name": "test-wt-cwd"}),
        ),
        chunks::tool_turn("tc-ls", "Ls", serde_json::json!({"path": "."})),
        chunks::text_turn("Cwd switch verified."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .cwd(git.path().to_path_buf())
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "EnterWorktree", false);
    assertions::assert_has_tool_result(&evts, "Ls", false);

    let wt_path = git.path().join(".loopal/worktrees/test-wt-cwd");
    assert!(wt_path.exists(), "worktree path should exist");
}

#[tokio::test]
async fn test_worktree_exit_keep() {
    let git = GitFixture::new();
    let calls = vec![
        chunks::tool_turn(
            "tc-enter",
            "EnterWorktree",
            serde_json::json!({"name": "test-wt-keep"}),
        ),
        chunks::tool_turn(
            "tc-exit",
            "ExitWorktree",
            serde_json::json!({"action": "keep"}),
        ),
        chunks::text_turn("Worktree kept."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .cwd(git.path().to_path_buf())
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "EnterWorktree", false);
    assertions::assert_has_tool_result(&evts, "ExitWorktree", false);

    let wt_path = git.path().join(".loopal/worktrees/test-wt-keep");
    assert!(wt_path.exists(), "worktree should still exist after keep");

    let results = extract_result(&evts, "ExitWorktree");
    assert!(
        results.iter().any(|r| r.contains("kept")),
        "exit result should mention 'kept', got: {results:?}"
    );
}

#[tokio::test]
async fn test_worktree_exit_remove() {
    let git = GitFixture::new();
    let calls = vec![
        chunks::tool_turn(
            "tc-enter",
            "EnterWorktree",
            serde_json::json!({"name": "test-wt-rm"}),
        ),
        chunks::tool_turn(
            "tc-exit",
            "ExitWorktree",
            serde_json::json!({"action": "remove"}),
        ),
        chunks::text_turn("Worktree removed."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .cwd(git.path().to_path_buf())
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "EnterWorktree", false);
    assertions::assert_has_tool_result(&evts, "ExitWorktree", false);

    let wt_path = git.path().join(".loopal/worktrees/test-wt-rm");
    assert!(
        !wt_path.exists(),
        "worktree should be removed after exit-remove"
    );

    let results = extract_result(&evts, "ExitWorktree");
    assert!(
        results.iter().any(|r| r.contains("removed")),
        "exit result should mention 'removed', got: {results:?}"
    );
}
