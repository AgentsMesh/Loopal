//! E2E for ModelSwitch / ThinkingSwitch / cold-start observable seeding.
//!
//! Lives in a separate file from `e2e_control_test.rs` because the
//! lifecycle suite asserts on `observable.*` mutator output; the
//! existing control suite only proved control commands didn't crash
//! the runner, which is what let `/clear` silently rot for a release
//! cycle.

use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, assertions, scenarios};
use loopal_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

fn wrap_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
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

#[tokio::test]
async fn test_cold_start_emits_initial_observable_seed() {
    let calls = scenarios::two_turn("hi.", "bye.");
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    let evts = harness.collect_until_idle().await;

    let has_model_changed = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ModelChanged { .. }));
    let has_mode_changed = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ModeChanged { .. }));
    let has_thinking = evts
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ThinkingChanged { .. }));
    assert!(has_model_changed, "cold-start must emit ModelChanged");
    assert!(has_mode_changed, "cold-start must emit ModeChanged");
    assert!(has_thinking, "cold-start must emit ThinkingChanged");

    let obs = harness.app.observable_for("main");
    assert!(
        !obs.model.is_empty(),
        "observable.model must be seeded by cold-start, got empty string"
    );
    assert!(
        matches!(obs.mode.as_str(), "act" | "plan"),
        "observable.mode must be normalized to a known label, got {:?}",
        obs.mode
    );
    assert!(
        !obs.thinking_config.is_empty(),
        "observable.thinking_config must be seeded by cold-start"
    );
}

#[tokio::test]
async fn test_model_switch_command_updates_observable() {
    let calls = scenarios::n_turn(&["before.", "after."]);
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    let initial = harness.app.observable_for("main").model.clone();
    harness
        .inner
        .control_tx
        .send(ControlCommand::ModelSwitch("claude-opus-4-7".into()))
        .await
        .unwrap();
    let evts = harness
        .collect_until(|e| matches!(e, AgentEventPayload::ModelChanged { .. }))
        .await;
    let saw_model_changed = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::ModelChanged { model } if model == "claude-opus-4-7"
        )
    });
    assert!(
        saw_model_changed,
        "expected ModelChanged event after ModelSwitch control"
    );
    let obs = harness.app.observable_for("main");
    assert_eq!(
        obs.model, "claude-opus-4-7",
        "observable.model must reflect ModelSwitch (was {initial:?})"
    );
}

#[tokio::test]
async fn test_thinking_switch_command_updates_observable() {
    let calls = scenarios::two_turn("Before switch.", "After switch.");
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    let json = serde_json::json!({"type": "disabled"}).to_string();
    harness
        .inner
        .control_tx
        .send(ControlCommand::ThinkingSwitch(json))
        .await
        .unwrap();
    let _ = harness
        .collect_until(|e| matches!(e, AgentEventPayload::ThinkingChanged { .. }))
        .await;
    assert_eq!(
        harness.app.observable_for("main").thinking_config,
        "disabled",
        "observable.thinking_config must reflect ThinkingSwitch"
    );

    // Ensure the runner is still healthy after the switch.
    harness
        .inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "hello"))
        .await
        .unwrap();
    let _ = harness.collect_until_idle().await;
    harness
        .inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "go"))
        .await
        .unwrap();
    let ev = harness.collect_until_idle().await;
    assertions::assert_has_stream(&ev);
}

#[tokio::test]
async fn test_permission_mode_switch_command_updates_observable() {
    let calls = scenarios::n_turn(&["before.", "after."]);
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    harness
        .inner
        .control_tx
        .send(ControlCommand::PermissionModeSwitch("ask_any_write".into()))
        .await
        .unwrap();
    let _ = harness
        .collect_until(|e| matches!(e, AgentEventPayload::PermissionModeChanged { .. }))
        .await;
    assert_eq!(
        harness.app.observable_for("main").permission_mode,
        "ask_any_write",
        "observable.permission_mode must reflect PermissionModeSwitch"
    );
}

#[tokio::test]
async fn test_decision_mode_switch_command_updates_observable() {
    let calls = scenarios::n_turn(&["before.", "after."]);
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    harness
        .inner
        .control_tx
        .send(ControlCommand::DecisionModeSwitch("classifier".into()))
        .await
        .unwrap();
    let _ = harness
        .collect_until(|e| matches!(e, AgentEventPayload::DecisionModeChanged { .. }))
        .await;
    assert_eq!(
        harness.app.observable_for("main").decision_mode,
        "classifier",
        "observable.decision_mode must reflect DecisionModeSwitch"
    );
}

#[tokio::test]
async fn test_sandbox_policy_switch_command_updates_observable() {
    let calls = scenarios::n_turn(&["before.", "after."]);
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    harness
        .inner
        .control_tx
        .send(ControlCommand::SandboxPolicySwitch("read_only".into()))
        .await
        .unwrap();
    let _ = harness
        .collect_until(|e| matches!(e, AgentEventPayload::SandboxPolicyChanged { .. }))
        .await;
    assert_eq!(
        harness.app.observable_for("main").sandbox_policy,
        "read_only",
        "observable.sandbox_policy must reflect SandboxPolicySwitch"
    );
}
