use loopal_context::{compact_messages, find_largest_result_block, truncate_block_content};
use loopal_message::{ContentBlock, Message, MessageRole};

#[test]
fn test_compact_keeps_system_and_last_n() {
    let mut msgs = vec![
        Message::system("sys"),
        Message::user("a"),
        Message::assistant("b"),
        Message::user("c"),
        Message::assistant("d"),
    ];
    compact_messages(&mut msgs, 2);
    assert_eq!(msgs.len(), 3); // system + last 2
    assert_eq!(msgs[0].role, MessageRole::System);
    assert_eq!(msgs[1].text_content(), "c");
    assert_eq!(msgs[2].text_content(), "d");
}

#[test]
fn test_compact_no_op_when_short() {
    let mut msgs = vec![Message::user("a"), Message::assistant("b")];
    compact_messages(&mut msgs, 5);
    assert_eq!(msgs.len(), 2);
}

#[test]
fn test_compact_no_system_messages() {
    let mut msgs = vec![
        Message::user("a"),
        Message::assistant("b"),
        Message::user("c"),
        Message::assistant("d"),
    ];
    compact_messages(&mut msgs, 2);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].text_content(), "c");
    assert_eq!(msgs[1].text_content(), "d");
}

#[test]
fn test_compact_exactly_at_limit() {
    // Messages exactly at keep_last + 1, should not compact
    // L5: messages.len() <= keep_last + 1 is true
    let mut msgs = vec![Message::user("a"), Message::assistant("b")];
    compact_messages(&mut msgs, 2);
    assert_eq!(msgs.len(), 2);
}

#[test]
fn test_compact_system_messages_with_few_non_system() {
    // L16: non_system_len <= keep_last is true
    let mut msgs = vec![
        Message::system("sys1"),
        Message::system("sys2"),
        Message::user("a"),
        Message::assistant("b"),
    ];
    compact_messages(&mut msgs, 3);
    // 4 total messages, keep_last=3, so len > keep_last + 1.
    // But non_system = 2, keep_last = 3, so non_system <= keep_last, no compaction.
    assert_eq!(msgs.len(), 4);
}

#[test]
fn test_compact_single_message() {
    let mut msgs = vec![Message::user("only")];
    compact_messages(&mut msgs, 5);
    assert_eq!(msgs.len(), 1);
}

#[test]
fn test_compact_keep_zero() {
    // Edge case: keep_last = 0
    let mut msgs = vec![
        Message::system("sys"),
        Message::user("a"),
        Message::assistant("b"),
    ];
    compact_messages(&mut msgs, 0);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].role, MessageRole::System);
}

// =============================================================================
// find_largest_tool_result tests
// =============================================================================

fn tool_result_message(id: &str, content: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            content: content.to_string(),
            is_error: false,
        }],
    }
}

#[test]
fn test_find_largest_tool_result_basic() {
    let msgs = vec![
        tool_result_message("a", "small"),
        tool_result_message("b", &"x".repeat(5000)),
        tool_result_message("c", "medium text here"),
    ];
    let (mi, bi, size) = find_largest_result_block(&msgs).unwrap();
    assert_eq!(mi, 1);
    assert_eq!(bi, 0);
    assert_eq!(size, 5000);
}

#[test]
fn test_find_largest_tool_result_empty() {
    let msgs = vec![Message::user("hello")];
    assert!(find_largest_result_block(&msgs).is_none());
}

#[test]
fn test_find_largest_tool_result_no_messages() {
    assert!(find_largest_result_block(&[]).is_none());
}

// =============================================================================
// truncate_block_content tests
// =============================================================================

#[test]
fn test_truncate_block_content_within_limits() {
    let mut block = ContentBlock::ToolResult {
        tool_use_id: "t1".into(),
        content: "short".into(),
        is_error: false,
    };
    truncate_block_content(&mut block, 100, 10_000);
    if let ContentBlock::ToolResult { content, .. } = &block {
        assert_eq!(content, "short");
    }
}

#[test]
fn test_truncate_block_content_by_bytes() {
    let big = "abcdefghij\n".repeat(100); // 1100 bytes
    let mut block = ContentBlock::ToolResult {
        tool_use_id: "t2".into(),
        content: big,
        is_error: false,
    };
    truncate_block_content(&mut block, 1000, 500);
    if let ContentBlock::ToolResult { content, .. } = &block {
        assert!(content.len() < 600);
        assert!(content.contains("[Truncated:"));
    }
}

#[test]
fn test_truncate_block_content_by_lines() {
    let big = (0..200)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut block = ContentBlock::ToolResult {
        tool_use_id: "t3".into(),
        content: big,
        is_error: false,
    };
    truncate_block_content(&mut block, 10, 100_000);
    if let ContentBlock::ToolResult { content, .. } = &block {
        assert!(content.contains("[Truncated:"));
        // Should have roughly 10 content lines + 2 truncation lines
        assert!(content.lines().count() <= 14);
    }
}

#[test]
fn test_truncate_block_content_ignores_non_tool_result() {
    let mut block = ContentBlock::Text {
        text: "x".repeat(10_000),
    };
    truncate_block_content(&mut block, 10, 500);
    // Should not modify Text blocks
    if let ContentBlock::Text { text } = &block {
        assert_eq!(text.len(), 10_000);
    }
}
