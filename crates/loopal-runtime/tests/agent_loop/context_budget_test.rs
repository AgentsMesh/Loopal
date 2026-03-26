use std::time::Duration;

use loopal_protocol::ControlCommand;

// --- effective_context_window tests ---

#[test]
fn test_effective_context_window_auto() {
    // cap=0 → use model's raw window
    let (runner, _) = super::make_runner();
    assert_eq!(
        runner.model_config.effective_context_window(),
        runner.model_config.max_context_tokens,
    );
}

#[test]
fn test_effective_context_window_capped() {
    // cap < model_window → use cap
    let (runner, _) = super::make_runner();
    let mut mc = runner.model_config.clone();
    mc.context_tokens_cap = 100_000;
    assert_eq!(mc.effective_context_window(), 100_000);
}

#[test]
fn test_effective_context_window_cap_larger_than_model() {
    // cap > model_window → use model_window (min semantics)
    let (runner, _) = super::make_runner();
    let mut mc = runner.model_config.clone();
    mc.context_tokens_cap = 500_000;
    assert_eq!(mc.effective_context_window(), mc.max_context_tokens);
}

#[test]
fn test_build_budget_uses_effective_window() {
    let (runner, _) = super::make_runner();
    let mut mc = runner.model_config.clone();

    // auto: budget.context_window == model's raw window
    let budget_auto = mc.build_budget("sys", 0);
    assert_eq!(budget_auto.context_window, mc.max_context_tokens);

    // capped: budget.context_window == cap
    mc.context_tokens_cap = 100_000;
    let budget_capped = mc.build_budget("sys", 0);
    assert_eq!(budget_capped.context_window, 100_000);
    assert!(budget_capped.message_budget < budget_auto.message_budget);
}

// --- model switch budget integration ---

#[tokio::test]
async fn test_model_switch_updates_budget() {
    let (mut runner, _event_rx, _mbox_tx, ctrl_tx, _perm_tx) = super::make_runner_with_channels();

    let original_window = runner.params.store.budget().context_window;

    ctrl_tx
        .send(ControlCommand::ModelSwitch("claude-sonnet-4-6".into()))
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    let new_window = runner.params.store.budget().context_window;
    // Sonnet 4.6 has 1M window; budget should reflect that
    assert!(
        new_window > original_window,
        "budget should grow after switching to larger model: {new_window} vs {original_window}"
    );
}

#[tokio::test]
async fn test_model_switch_preserves_cap() {
    let (mut runner, _event_rx, _mbox_tx, ctrl_tx, _perm_tx) = super::make_runner_with_channels();

    // Simulate a user-configured cap of 300K
    runner.model_config.context_tokens_cap = 300_000;

    ctrl_tx
        .send(ControlCommand::ModelSwitch("claude-sonnet-4-6".into()))
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    // Cap should be preserved across model switch
    assert_eq!(runner.model_config.context_tokens_cap, 300_000);
    // Budget should use capped value (300K), not model's 1M
    let window = runner.params.store.budget().context_window;
    assert_eq!(window, 300_000);
}
