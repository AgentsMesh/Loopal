//! E2E hook tests: pre-tool hook execution, failure handling, output capture.
//!
//! Hooks use shell scripts (`sh -c`) and are Unix-only.
#![cfg(unix)]

use std::time::Duration;

use loopal_config::{HookConfig, HookEvent};
use loopal_test_support::{HarnessBuilder, HookFixture, assertions, chunks};
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

#[tokio::test]
async fn test_pre_tool_hook_executes() {
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("hook_executed");

    let hooks = vec![HookConfig {
        event: HookEvent::PreToolUse,
        command: script.to_str().unwrap().to_string(),
        tool_filter: None,
        timeout_ms: 5000,
    }];

    let calls = vec![
        chunks::tool_turn(
            "tc-r",
            "Read",
            serde_json::json!({"file_path": "/dev/null"}),
        ),
        chunks::text_turn("Hook test done."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .hooks(hooks)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "Read", false);

    // Wait briefly for hook I/O to flush
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Marker file should have been created by the pre-hook script
    assert!(
        marker.exists(),
        "pre-hook marker file should exist at {}",
        marker.display()
    );
    let content = std::fs::read_to_string(&marker).unwrap();
    assert!(
        content.contains("hook_executed"),
        "marker should contain 'hook_executed', got: {content}"
    );
}

#[tokio::test]
async fn test_hook_failure_blocks_tool() {
    // A failing pre-hook (exit 1) should block the tool and return an error result.
    let mut hook_fx = HookFixture::new();
    let script = hook_fx.create_failing_hook();

    let hooks = vec![HookConfig {
        event: HookEvent::PreToolUse,
        command: script.to_str().unwrap().to_string(),
        tool_filter: None,
        timeout_ms: 5000,
    }];

    let calls = vec![
        chunks::tool_turn(
            "tc-r",
            "Read",
            serde_json::json!({"file_path": "/dev/null"}),
        ),
        chunks::text_turn("Hook failed."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .hooks(hooks)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    // Failing pre-hook → tool result is an error (pre-hook rejected)
    assertions::assert_has_tool_result(&evts, "Read", true);
    assertions::assert_has_stream(&evts);
}

#[tokio::test]
async fn test_post_hook_output_captured() {
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("post_hook_ran");

    let hooks = vec![HookConfig {
        event: HookEvent::PostToolUse,
        command: script.to_str().unwrap().to_string(),
        tool_filter: None,
        timeout_ms: 5000,
    }];

    let calls = vec![
        chunks::tool_turn(
            "tc-r",
            "Read",
            serde_json::json!({"file_path": "/dev/null"}),
        ),
        chunks::text_turn("Post hook test."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .hooks(hooks)
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "Read", false);

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Post-hook should have created the marker
    assert!(
        marker.exists(),
        "post-hook marker file should exist at {}",
        marker.display()
    );
    let content = std::fs::read_to_string(&marker).unwrap();
    assert!(
        content.contains("post_hook_ran"),
        "marker should contain 'post_hook_ran', got: {content}"
    );
}
