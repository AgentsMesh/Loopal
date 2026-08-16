#![cfg(unix)]

use std::time::Duration;

use loopal_config::{HookConfig, HookEvent};
use loopal_context::ContextBudget;
use loopal_provider_api::Message;
use loopal_test_support::{HarnessBuilder, HookFixture, chunks};

const TEST_HOOK_TIMEOUT_MS: u64 = 30_000;

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
    // cl100k_base compresses uniform runs to nothing; varied bytes give a
    // realistic token count.
    let body: String = (0..100u8).map(|i| char::from(b'a' + (i % 26))).collect();
    Message::user(&format!("{label}: {body}"))
}

fn build_hook(script: &std::path::Path) -> HookConfig {
    HookConfig {
        event: HookEvent::PreCompact,
        command: script.to_str().unwrap().to_string(),
        tool_filter: None,
        timeout_ms: TEST_HOOK_TIMEOUT_MS,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }
}

async fn wait_for_marker(path: &std::path::Path, expected: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !std::fs::read_to_string(path).is_ok_and(|content| content.contains(expected)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("hook marker did not appear: {}", path.display()));
}

#[tokio::test]
async fn precompact_hook_fires_on_manual_compact() {
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("manual_compact_fired");

    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .hooks(vec![build_hook(&script)])
        .messages((0..6).map(|i| Message::user(&format!("m{i}"))).collect())
        .build()
        .await;

    h.runner.force_compact(None).await.unwrap();
    wait_for_marker(&marker, "manual_compact_fired").await;

    assert!(
        marker.exists(),
        "PreCompact hook marker must exist after force_compact, at {}",
        marker.display()
    );
    let content = std::fs::read_to_string(&marker).unwrap();
    assert!(
        content.contains("manual_compact_fired"),
        "marker should hold 'manual_compact_fired', got: {content}"
    );
}

#[tokio::test]
async fn precompact_hook_skipped_when_nothing_to_compact() {
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("should_not_fire");

    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .hooks(vec![build_hook(&script)])
        .messages(vec![Message::user("single")])
        .build()
        .await;

    h.runner.force_compact(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !marker.exists(),
        "PreCompact must not fire when conversation is too short"
    );
}

#[tokio::test]
async fn precompact_hook_fires_on_auto_compact() {
    // Auto path through check_and_compact must fire PreCompact symmetric
    // to manual /compact.
    let mut hook_fx = HookFixture::new();
    let (script, marker) = hook_fx.create_echo_hook("auto_compact_fired");

    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary")])
        .hooks(vec![build_hook(&script)])
        .messages((0..30).map(|i| padded(&format!("seed-{i}"))).collect())
        .build()
        .await;
    h.runner.turns.update_budget(tiny_budget());

    assert!(
        h.runner.turns.view().needs_summarization(),
        "precondition: store must already need compaction; effective_tokens={}",
        h.runner.turns.view().effective_tokens()
    );

    let cancel = tokio_util::sync::CancellationToken::new();
    h.runner.check_and_compact(&cancel).await.unwrap();
    wait_for_marker(&marker, "auto_compact_fired").await;

    assert!(
        marker.exists(),
        "PreCompact marker must exist after auto-compact, at {}",
        marker.display()
    );
    let content = std::fs::read_to_string(&marker).unwrap();
    assert!(
        content.contains("auto_compact_fired"),
        "marker should hold 'auto_compact_fired', got: {content}"
    );
}
