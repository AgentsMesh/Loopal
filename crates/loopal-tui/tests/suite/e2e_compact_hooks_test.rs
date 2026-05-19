#![cfg(unix)]

use std::time::Duration;

use loopal_config::{HookConfig, HookEvent};
use loopal_message::Message;
use loopal_test_support::{HarnessBuilder, HookFixture, chunks};

fn build_hook(script: &std::path::Path) -> HookConfig {
    HookConfig {
        event: HookEvent::PreCompact,
        command: script.to_str().unwrap().to_string(),
        tool_filter: None,
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }
}

async fn run_force_compact_with_hook(hook: HookConfig) {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .hooks(vec![hook])
        .build()
        .await;

    h.runner.params.store.clear();
    for i in 0..6 {
        h.runner
            .params
            .store
            .push_user(Message::user(&format!("m{i}")));
    }

    h.runner.force_compact(None).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;
}

/// `/compact` must fire `PreCompact` hooks so audit / telemetry / snapshot
/// integrators see manual compaction too — not just the auto path.
#[tokio::test]
async fn precompact_hook_fires_on_manual_compact() {
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("manual_compact_fired");

    run_force_compact_with_hook(build_hook(&script)).await;

    assert!(
        marker.exists(),
        "PreCompact hook marker must exist after force_compact, looked at {}",
        marker.display(),
    );
    let content = std::fs::read_to_string(&marker).unwrap();
    assert!(
        content.contains("manual_compact_fired"),
        "marker should hold 'manual_compact_fired', got: {content}",
    );
}

/// Short conversations skip both compaction and the `PreCompact` hook —
/// there is nothing to gate.
#[tokio::test]
async fn precompact_hook_skipped_when_nothing_to_compact() {
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("should_not_fire");

    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .hooks(vec![build_hook(&script)])
        .build()
        .await;

    h.runner.params.store.clear();
    h.runner.params.store.push_user(Message::user("single"));

    h.runner.force_compact(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !marker.exists(),
        "PreCompact must not fire when the conversation is too short",
    );
}
