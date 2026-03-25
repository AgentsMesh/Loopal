//! E2E permission tests: supervised approve/deny, bypass auto-allows, render check.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, TestFixture, assertions, chunks};
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

fn build_custom_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
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

async fn collect_until_perm(harness: &mut TuiTestHarness) -> Vec<AgentEventPayload> {
    let mut all_events = Vec::new();
    let timeout = std::time::Duration::from_secs(10);
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, harness.inner.event_rx.recv()).await {
            Ok(Some(event)) => {
                let is_perm = matches!(
                    &event.payload,
                    AgentEventPayload::ToolPermissionRequest { .. }
                );
                harness.app.session.handle_event(event.clone());
                all_events.push(event.payload);
                if is_perm {
                    break;
                }
            }
            Ok(None) => panic!("channel closed before ToolPermissionRequest"),
            Err(_) => panic!("timeout waiting for ToolPermissionRequest"),
        }
    }
    all_events
}

#[tokio::test]
async fn test_supervised_approve() {
    let fixture = TestFixture::new();
    let path_str = fixture.path().join("perm_test.txt");
    let path_str = path_str.to_str().unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": path_str, "content": "supervised content"}),
        ),
        chunks::text_turn("Write approved."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .permission_mode(loopal_tool_api::PermissionMode::Supervised)
        .interactive(true)
        .build_spawned()
        .await;

    let mut harness = build_custom_tui(inner);
    let mut all_events = collect_until_perm(&mut harness).await;
    harness.inner.session_ctrl.approve_permission().await;
    let rest = harness.collect_until_idle().await;
    all_events.extend(rest);

    assertions::assert_has_tool_result(&all_events, "Write", false);
    assertions::assert_has_stream(&all_events);
}

#[tokio::test]
async fn test_supervised_deny() {
    let fixture = TestFixture::new();
    let path_str = fixture.path().join("deny_test.txt");
    let path_str = path_str.to_str().unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": path_str, "content": "denied content"}),
        ),
        chunks::text_turn("Write was denied."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .permission_mode(loopal_tool_api::PermissionMode::Supervised)
        .interactive(true)
        .build_spawned()
        .await;

    let mut harness = build_custom_tui(inner);
    let mut all_events = collect_until_perm(&mut harness).await;
    harness.inner.session_ctrl.deny_permission().await;
    let rest = harness.collect_until_idle().await;
    all_events.extend(rest);

    assertions::assert_has_tool_result(&all_events, "Write", true);
}

#[tokio::test]
async fn test_bypass_auto_allows() {
    let fixture = TestFixture::new();
    let path_str = fixture.path().join("bypass_test.txt");
    let path_str = path_str.to_str().unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": path_str, "content": "bypass content"}),
        ),
        chunks::text_turn("Write done."),
    ];
    let mut harness = super::e2e_harness::build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    let has_perm = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ToolPermissionRequest { .. }));
    assert!(
        !has_perm,
        "bypass mode should not emit ToolPermissionRequest"
    );
    assertions::assert_has_tool_result(&evts, "Write", false);
}

#[tokio::test]
async fn test_permission_dialog_render() {
    let fixture = TestFixture::new();
    let path_str = fixture.path().join("render_perm.txt");
    let path_str = path_str.to_str().unwrap();

    let calls = vec![
        chunks::tool_turn(
            "tc-w",
            "Write",
            serde_json::json!({"file_path": path_str, "content": "render test"}),
        ),
        chunks::text_turn("Rendered."),
    ];
    let inner = HarnessBuilder::new()
        .calls(calls)
        .permission_mode(loopal_tool_api::PermissionMode::Supervised)
        .interactive(true)
        .build_spawned()
        .await;

    let mut harness = build_custom_tui(inner);
    let _evts = collect_until_perm(&mut harness).await;

    // Render the TUI while permission dialog is pending
    let text = harness.render_text();
    // The rendered output should show the tool name being requested
    assert!(
        text.contains("Write"),
        "rendered TUI should show the tool name 'Write' in permission dialog, got:\n{text}"
    );

    // Approve so the loop can finish cleanly
    harness.inner.session_ctrl.approve_permission().await;
    let _ = harness.collect_until_idle().await;
}
