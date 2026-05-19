#![cfg(unix)]

use std::time::Duration;

use loopal_config::{HookConfig, HookEvent};
use loopal_context::ContextBudget;
use loopal_message::Message;
use loopal_test_support::{HarnessBuilder, HookFixture, chunks};

fn tiny_budget() -> ContextBudget {
    ContextBudget {
        context_window: 500,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 50,
        safety_margin: 25,
        message_budget: 425,
        max_output_tokens: 50,
    }
}

fn padded(label: &str) -> Message {
    // cl100k_base compresses long runs of the same byte to almost
    // nothing, so a uniform payload would slip under the compaction
    // threshold. Varied bytes give a realistic token count.
    let body: String = (0..100u8).map(|i| char::from(b'a' + (i % 26))).collect();
    Message::user(&format!("{label}: {body}"))
}

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

#[tokio::test]
async fn precompact_hook_fires_on_auto_compact() {
    // Auto path goes through `check_and_compact` (token-threshold), which
    // must fire PreCompact symmetric to the manual `/compact` path.
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("auto_compact_fired");

    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary")])
        .hooks(vec![build_hook(&script)])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..30 {
        h.runner
            .params
            .store
            .push_user(padded(&format!("seed-{i}")));
    }
    assert!(
        h.runner.params.store.needs_summarization(),
        "precondition: store must already need compaction; effective_tokens={}",
        h.runner.params.store.effective_tokens(),
    );

    let cancel = tokio_util::sync::CancellationToken::new();
    h.runner.check_and_compact(&cancel).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        marker.exists(),
        "PreCompact hook marker must exist after auto-compact, looked at {}",
        marker.display(),
    );
    let content = std::fs::read_to_string(&marker).unwrap();
    assert!(
        content.contains("auto_compact_fired"),
        "marker should hold 'auto_compact_fired', got: {content}",
    );
}
