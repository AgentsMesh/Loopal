use loopal_context::{estimate_message_tokens, estimate_messages_tokens, estimate_tokens};
use loopal_message::{ContentBlock, Message, MessageRole};

#[test]
fn test_estimate_tokens_empty() {
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn test_estimate_tokens_hello_world() {
    // BPE encodes "hello world" as ["hello", " world"] → 2 tokens
    let tokens = estimate_tokens("hello world");
    assert!((2..=4).contains(&tokens), "got {tokens}");
}

#[test]
fn test_estimate_tokens_code_snippet() {
    // Code should produce a reasonable token count
    let code = "fn main() { println!(\"Hello, world!\"); }";
    let tokens = estimate_tokens(code);
    // BPE typically encodes this as ~15 tokens
    assert!(tokens > 5 && tokens < 30, "got {tokens}");
}

#[test]
fn test_estimate_tokens_chinese_text() {
    // CJK characters consume more tokens than ASCII per character.
    // With chars/4, "你好世界" (12 bytes) would be 3 tokens — far too low.
    let tokens = estimate_tokens("你好世界");
    assert!(tokens >= 4, "CJK should be at least 4 tokens, got {tokens}");
}

#[test]
fn test_estimate_message_tokens_single() {
    let msg = Message::user("hello world");
    let tokens = estimate_message_tokens(&msg);
    // BPE("hello world") + 4 overhead
    assert!((6..=10).contains(&tokens), "got {tokens}");
}

#[test]
fn test_estimate_messages_tokens_empty() {
    assert_eq!(estimate_messages_tokens(&[]), 0);
}

#[test]
fn test_estimate_messages_tokens_sum() {
    let msgs = vec![Message::user("hello"), Message::assistant("world")];
    let total = estimate_messages_tokens(&msgs);
    let sum = estimate_message_tokens(&msgs[0]) + estimate_message_tokens(&msgs[1]);
    assert_eq!(total, sum);
}

#[test]
fn test_estimate_messages_tokens_includes_tool_io() {
    let msgs = vec![
        Message {
            id: None,
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
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "fn main() {}".into(),
                is_error: false,
            }],
        },
    ];
    let total = estimate_messages_tokens(&msgs);
    // Should include text + tool input + tool result + overhead
    assert!(total > 15, "expected tool IO counted, got {total}");
}

#[test]
fn test_bpe_more_accurate_than_char_div_4() {
    // For common English text, BPE and chars/4 diverge significantly
    // on short tokens and punctuation-heavy text.
    let text = "I'm a developer! Let's go.";
    let bpe_tokens = estimate_tokens(text);
    let char_div_4 = text.len() as u32 / 4;
    // BPE should produce a different (typically higher) count for
    // punctuation-heavy text with contractions.
    assert_ne!(
        bpe_tokens, char_div_4,
        "BPE should differ from chars/4 heuristic"
    );
}
