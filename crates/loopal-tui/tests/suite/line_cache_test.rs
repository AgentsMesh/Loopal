use std::time::Instant;

use loopal_tui::views::progress::LineCache;
use loopal_view_state::{InvocationId, InvocationState, Outcome, SessionMessage, ToolInvocation};

const W: u16 = 80;

fn msg(role: &str, content: &str) -> SessionMessage {
    SessionMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }
}

fn pending_call(name: &str, summary: &str) -> ToolInvocation {
    ToolInvocation::start(
        InvocationId::new("tc-1").unwrap(),
        name,
        summary,
        None,
        Instant::now(),
    )
}

fn complete_with(content: &str) -> InvocationState {
    InvocationState::Done {
        duration: std::time::Duration::ZERO,
        outcome: Outcome::Success {
            content: content.to_string(),
        },
    }
}

#[test]
fn test_empty_messages() {
    let mut cache = LineCache::new();
    assert_eq!(cache.update(&[], W), 0);
    assert!(cache.slice(0, 100).is_empty());
}

#[test]
fn test_incremental_append() {
    let mut cache = LineCache::new();
    let msgs = vec![msg("user", "hello")];
    let n1 = cache.update(&msgs, W);
    assert!(n1 > 0);

    let msgs = vec![msg("user", "hello"), msg("assistant", "hi")];
    let n2 = cache.update(&msgs, W);
    assert!(n2 > n1);
}

#[test]
fn test_slice_window() {
    let mut cache = LineCache::new();
    let msgs: Vec<_> = (0..20).map(|i| msg("user", &format!("msg {i}"))).collect();
    cache.update(&msgs, W);
    let total = cache.total_lines();
    let start = total.saturating_sub(5);
    let tail = cache.slice(start, 5);
    assert!(tail.len() <= 5);
}

#[test]
fn test_clear_invalidation() {
    let mut cache = LineCache::new();
    let msgs = vec![msg("user", "hello"), msg("assistant", "hi")];
    cache.update(&msgs, W);
    cache.update(&[], W);
    assert!(cache.slice(0, 100).is_empty());
}

#[test]
fn test_tool_call_mutation_detected() {
    let mut cache = LineCache::new();
    let mut msgs = vec![SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![pending_call("bash", "bash(ls)")],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }];
    let fp1 = cache.update(&msgs, W);

    msgs[0].tool_calls[0].state = complete_with("done");
    let fp2 = cache.update(&msgs, W);

    let text: String = cache
        .slice(0, cache.total_lines())
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("●"), "should show ● icon after mutation");
    assert!(fp1 > 0 && fp2 > 0, "both updates should produce lines");
}

#[test]
fn test_width_change_triggers_full_rebuild() {
    let mut cache = LineCache::new();
    let long = "word ".repeat(30);
    let msgs = vec![msg("user", &long)];

    let n80 = cache.update(&msgs, 80);
    let n40 = cache.update(&msgs, 40);

    assert!(n40 > n80, "narrower width should produce more lines");
}

#[test]
fn test_same_width_preserves_cache() {
    let mut cache = LineCache::new();
    let msgs = vec![msg("user", "hello")];
    let n1 = cache.update(&msgs, W);
    let n2 = cache.update(&msgs, W);
    assert_eq!(n1, n2);
}

#[test]
fn test_tool_result_arrival_invalidates_cache() {
    let mut cache = LineCache::new();
    let mut msgs = vec![SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![pending_call("Read", "Read(/tmp/foo.rs)")],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }];
    let n1 = cache.update(&msgs, W);

    msgs[0].tool_calls[0].state = complete_with("file content here");
    let n2 = cache.update(&msgs, W);

    let text: String = cache
        .slice(0, cache.total_lines())
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("●"), "result arrival should update to ● icon");
    assert!(n1 > 0 && n2 > 0, "both states should produce lines");
}

#[test]
fn same_length_outcome_content_change_invalidates_cache() {
    let mut cache = LineCache::new();
    let mut msgs = vec![SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![pending_call("Bash", "Bash(echo)")],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }];
    msgs[0].tool_calls[0].state = complete_with("aaaa");
    cache.update(&msgs, W);
    let lines_a: Vec<String> = cache
        .slice(0, cache.total_lines())
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
        .collect();

    msgs[0].tool_calls[0].state = complete_with("bbbb");
    cache.update(&msgs, W);
    let lines_b: Vec<String> = cache
        .slice(0, cache.total_lines())
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
        .collect();

    assert_ne!(
        lines_a, lines_b,
        "same-length but different content must invalidate cache"
    );
}
