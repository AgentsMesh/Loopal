use loopal_session::types::{DisplayMessage, DisplayToolCall};
use loopal_tui::views::progress::LineCache;

const W: u16 = 80;

fn msg(role: &str, content: &str) -> DisplayMessage {
    DisplayMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
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
    let mut msgs = vec![DisplayMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![DisplayToolCall {
            name: "bash".to_string(),
            status: "pending".to_string(),
            summary: "bash(ls)".to_string(),
            result: None,
        }],
    }];
    cache.update(&msgs, W);
    msgs[0].tool_calls[0].status = "success".to_string();
    msgs[0].tool_calls[0].summary = "bash(ls)".to_string();
    msgs[0].tool_calls[0].result = Some("done".to_string());
    cache.update(&msgs, W);
    let text: String = cache
        .tail(100)
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("done"));
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
    let mut msgs = vec![DisplayMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![DisplayToolCall {
            name: "Read".to_string(),
            status: "pending".to_string(),
            summary: "Read(/tmp/foo.rs)".to_string(),
            result: None,
        }],
    }];
    let n1 = cache.update(&msgs, W);

    // Simulate ToolResult arrival — result changes from None to Some
    msgs[0].tool_calls[0].status = "success".to_string();
    msgs[0].tool_calls[0].result = Some("file content here".to_string());
    let n2 = cache.update(&msgs, W);

    // Cache should detect the mutation (more lines due to result rendering)
    assert!(n2 > n1, "result arrival should produce more lines");
}
