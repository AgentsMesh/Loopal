use loopal_context::{estimate_messages_tokens, estimate_tokens};
use loopal_types::message::{ContentBlock, Message, MessageRole};

#[test]
fn test_estimate_tokens_empty() {
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn test_estimate_tokens_short() {
    // 8 chars -> 2 tokens
    assert_eq!(estimate_tokens("abcdefgh"), 2);
}

#[test]
fn test_estimate_tokens_rounding_down() {
    // 5 chars -> 1 token (integer division)
    assert_eq!(estimate_tokens("abcde"), 1);
}

#[test]
fn test_estimate_messages_tokens() {
    let msgs = vec![Message::user("abcdefgh"), Message::assistant("abcd")];
    // user: 8/4 + 4 overhead = 6
    // assistant: 4/4 + 4 overhead = 5
    assert_eq!(estimate_messages_tokens(&msgs), 11);
}

#[test]
fn test_estimate_messages_tokens_empty() {
    assert_eq!(estimate_messages_tokens(&[]), 0);
}

#[test]
fn test_estimate_messages_tokens_includes_tool_io() {
    let msgs = vec![
        Message {
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me read the file.".into(),
                },
                ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({"file_path": "/tmp/test.rs"}),
                },
            ],
        },
        Message {
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "fn main() {}".into(),
                is_error: false,
            }],
        },
    ];
    let total = estimate_messages_tokens(&msgs);
    // Should be significantly more than just text tokens
    assert!(total > 10, "expected tool IO to be counted, got {}", total);
}
