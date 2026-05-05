use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, assertions, chunks};
use loopal_tui::app::App;
use loopal_tui::input::handle_key;
use loopal_tui::key_dispatch_for_test;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

fn build_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        inner.fixture.path().to_path_buf(),
    );
    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

async fn collect_until_question(harness: &mut TuiTestHarness) -> Vec<AgentEventPayload> {
    let mut all_events = Vec::new();
    let timeout = std::time::Duration::from_secs(10);
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, harness.inner.event_rx.recv()).await {
            Ok(Some(event)) => {
                let is_q = matches!(
                    &event.payload,
                    AgentEventPayload::UserQuestionRequest { .. }
                );
                let payload = event.payload.clone();
                super::e2e_harness::dispatch_to_app(&mut harness.app, event);
                all_events.push(payload);
                if is_q {
                    break;
                }
            }
            Ok(None) => panic!("channel closed before UserQuestionRequest"),
            Err(_) => panic!("timeout waiting for UserQuestionRequest"),
        }
    }
    all_events
}

fn assert_q_prefix_and_answer(result: &str, expected_q_index: usize, expected_answer: &str) {
    let prefix = format!("Q{expected_q_index} (");
    assert!(
        result.starts_with(&prefix),
        "expected prefix {prefix:?}, got: {result}"
    );
    let (_, ans) = result
        .rsplit_once(": ")
        .unwrap_or_else(|| panic!("missing ': ' separator in: {result}"));
    assert_eq!(ans, expected_answer, "answer mismatch in: {result}");
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[tokio::test]
async fn askuser_silent_enter_without_interaction_does_not_submit() {
    let calls = vec![
        chunks::tool_turn(
            "tc-q",
            "AskUser",
            serde_json::json!({"questions": [{
                "question": "Pick one",
                "options": [
                    {"label": "Yes", "description": ""},
                    {"label": "No", "description": ""},
                ],
            }]}),
        ),
        chunks::text_turn("Got it."),
    ];
    let inner = HarnessBuilder::new().calls(calls).build_spawned().await;
    let mut harness = build_tui(inner);
    let _ = collect_until_question(&mut harness).await;

    // 用户没动 cursor 直接 Enter — 应该被防御不提交
    let action = handle_key(&mut harness.app, key(KeyCode::Enter));
    key_dispatch_for_test::dispatch(&mut harness.app, action).await;

    // pending_question 应仍存在
    let still_pending = harness
        .app
        .with_active_conversation(|conv| conv.pending_question.is_some());
    assert!(
        still_pending,
        "silent Enter must NOT submit when user has not interacted"
    );

    // transient_status 应有提示
    let status = harness.app.current_transient_status().map(String::from);
    assert!(
        status.as_deref().is_some_and(|s| s.contains("Press")),
        "transient_status should hint to press arrow keys, got: {status:?}"
    );
}

#[tokio::test]
async fn askuser_single_select_via_key_dispatch_passes_label_to_llm() {
    let calls = vec![
        chunks::tool_turn(
            "tc-q",
            "AskUser",
            serde_json::json!({"questions": [{
                "question": "Pick one",
                "options": [
                    {"label": "Yes", "description": ""},
                    {"label": "No", "description": ""},
                ],
            }]}),
        ),
        chunks::text_turn("Got it."),
    ];
    let inner = HarnessBuilder::new().calls(calls).build_spawned().await;
    let mut harness = build_tui(inner);
    let mut events = collect_until_question(&mut harness).await;

    // simulate user interaction: arrow down + up to acknowledge before Enter
    let action = handle_key(&mut harness.app, key(KeyCode::Down));
    key_dispatch_for_test::dispatch(&mut harness.app, action).await;
    let action = handle_key(&mut harness.app, key(KeyCode::Up));
    key_dispatch_for_test::dispatch(&mut harness.app, action).await;

    let action = handle_key(&mut harness.app, key(KeyCode::Enter));
    key_dispatch_for_test::dispatch(&mut harness.app, action).await;

    let rest = harness.collect_until_idle().await;
    events.extend(rest);

    let result =
        assertions::find_tool_result(&events, "AskUser").expect("AskUser ToolResult event missing");
    assert_q_prefix_and_answer(&result, 1, "Yes");
}

#[tokio::test]
async fn askuser_cancel_via_esc_yields_cancelled_token() {
    let calls = vec![
        chunks::tool_turn(
            "tc-q",
            "AskUser",
            serde_json::json!({"questions": [{
                "question": "Choose",
                "options": [{"label": "A", "description": ""}],
            }]}),
        ),
        chunks::text_turn("Cancelled handled."),
    ];
    let inner = HarnessBuilder::new().calls(calls).build_spawned().await;
    let mut harness = build_tui(inner);
    let mut events = collect_until_question(&mut harness).await;

    let action = handle_key(&mut harness.app, key(KeyCode::Esc));
    key_dispatch_for_test::dispatch(&mut harness.app, action).await;

    let rest = harness.collect_until_idle().await;
    events.extend(rest);

    let result =
        assertions::find_tool_result(&events, "AskUser").expect("AskUser ToolResult event missing");
    assert_eq!(result, "(cancelled by user)");
}

#[tokio::test]
async fn askuser_other_free_text_via_typing_passes_text_to_llm() {
    let calls = vec![
        chunks::tool_turn(
            "tc-q",
            "AskUser",
            serde_json::json!({"questions": [{
                "question": "Type your answer",
                "options": [],
            }]}),
        ),
        chunks::text_turn("ack."),
    ];
    let inner = HarnessBuilder::new().calls(calls).build_spawned().await;
    let mut harness = build_tui(inner);
    let mut events = collect_until_question(&mut harness).await;

    for c in "hello".chars() {
        let action = handle_key(&mut harness.app, key(KeyCode::Char(c)));
        key_dispatch_for_test::dispatch(&mut harness.app, action).await;
    }
    let action = handle_key(&mut harness.app, key(KeyCode::Enter));
    key_dispatch_for_test::dispatch(&mut harness.app, action).await;

    let rest = harness.collect_until_idle().await;
    events.extend(rest);

    let result =
        assertions::find_tool_result(&events, "AskUser").expect("AskUser ToolResult event missing");
    assert_q_prefix_and_answer(&result, 1, "hello");
}
