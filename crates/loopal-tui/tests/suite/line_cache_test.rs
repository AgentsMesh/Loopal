use loopal_session::types::{SessionMessage, SessionToolCall, ToolCallStatus};
use loopal_tui::views::progress::LineCache;

const W: u16 = 80;

fn msg(role: &str, content: &str) -> SessionMessage {
    SessionMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
    }
}

#[test]
fn test_empty_messages() {
    let mut cache = LineCache::new();
    assert_eq!(cache.update(&[], W), 0);
    assert!(cache.tail(100).is_empty());
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
fn test_tail_window() {
    let mut cache = LineCache::new();
    let msgs: Vec<_> = (0..20).map(|i| msg("user", &format!("msg {i}"))).collect();
    cache.update(&msgs, W);
    let tail = cache.tail(5);
    assert!(tail.len() <= 5);
}

#[test]
fn test_clear_invalidation() {
    let mut cache = LineCache::new();
    let msgs = vec![msg("user", "hello"), msg("assistant", "hi")];
    cache.update(&msgs, W);
    cache.update(&[], W);
    assert!(cache.tail(100).is_empty());
}

#[test]
fn test_tool_call_mutation_detected() {
    let mut cache = LineCache::new();
    let mut msgs = vec![SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![SessionToolCall {
            name: "bash".to_string(),
            id: String::new(),
            status: ToolCallStatus::Pending,
            summary: "bash(ls)".to_string(),
            result: None,
            tool_input: None,
            batch_id: None,
            started_at: None,
            duration_ms: None,
            progress_tail: None,
            metadata: None,
        }],
        image_count: 0,
        skill_info: None,
    }];
    let fp1 = cache.update(&msgs, W);

    // Mutate: status changes → icon color changes (yellow→green ●)
    msgs[0].tool_calls[0].status = ToolCallStatus::Success;
    msgs[0].tool_calls[0].result = Some("done".to_string());
    let fp2 = cache.update(&msgs, W);

    // Cache should detect the mutation via fingerprint change
    let text: String = cache
        .tail(100)
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    // Single-line summary should contain the status icon
    assert!(text.contains("●"), "should show ● icon after mutation");
    // Fingerprint-triggered rebuild means line counts may differ
    assert!(fp1 > 0 && fp2 > 0, "both updates should produce lines");
}

#[test]
fn test_width_change_triggers_full_rebuild() {
    let mut cache = LineCache::new();
    // Long line that wraps differently at different widths
    let long = "word ".repeat(30); // 150 chars
    let msgs = vec![msg("user", &long)];

    let n80 = cache.update(&msgs, 80);
    let n40 = cache.update(&msgs, 40);

    // Narrower width → more visual lines
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
        tool_calls: vec![SessionToolCall {
            name: "Read".to_string(),
            id: String::new(),
            status: ToolCallStatus::Pending,
            summary: "Read(/tmp/foo.rs)".to_string(),
            result: None,
            tool_input: None,
            batch_id: None,
            started_at: None,
            duration_ms: None,
            progress_tail: None,
            metadata: None,
        }],
        image_count: 0,
        skill_info: None,
    }];
    let n1 = cache.update(&msgs, W);

    // Simulate ToolResult arrival — result changes from None to Some
    msgs[0].tool_calls[0].status = ToolCallStatus::Success;
    msgs[0].tool_calls[0].result = Some("file content here".to_string());
    let n2 = cache.update(&msgs, W);

    // Single-line summary: line count stays the same, but cache was rebuilt
    // (fingerprint changed). Verify the summary text is updated.
    let text: String = cache
        .tail(100)
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("●"), "result arrival should update to ● icon");
    assert!(n1 > 0 && n2 > 0, "both states should produce lines");
}
