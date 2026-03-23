use loopal_context::middleware::SmartCompact;
use loopal_message::{ContentBlock, Message};
use loopal_provider_api::{Middleware, MiddlewareContext};

fn make_ctx(messages: Vec<Message>, max_context_tokens: u32) -> MiddlewareContext {
    MiddlewareContext {
        messages,
        system_prompt: String::new(),
        model: "test-model".into(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost: 0.0,
        max_context_tokens,
        compact_model: None,
        summarization_provider: None,
    }
}

fn large_message(n: usize) -> Message {
    Message::user(&"x".repeat(n))
}

#[tokio::test]
async fn smart_compact_few_messages_no_change_even_over_limit() {
    let keep_last = 5;
    let mw = SmartCompact::new(keep_last);
    let messages = vec![large_message(400), large_message(400), large_message(400)];
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 50);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), original_len);
}

#[tokio::test]
async fn smart_compact_name() {
    let mw = SmartCompact::new(5);
    assert_eq!(mw.name(), "smart_compact");
}

#[tokio::test]
async fn smart_compact_with_tool_io_messages() {
    let mw = SmartCompact::new(2);

    let tool_use_msg = Message {
        id: None,
        role: loopal_message::MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tool_1".to_string(),
            name: "Bash".to_string(),
            input: serde_json::json!({"command": "echo hello"}),
        }],
    };

    let tool_result_msg = Message {
        id: None,
        role: loopal_message::MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: "hello\n".to_string(),
            is_error: false,
        }],
    };

    let error_tool_result_msg = Message {
        id: None,
        role: loopal_message::MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tool_2".to_string(),
            content: "command not found".to_string(),
            is_error: true,
        }],
    };

    let mut messages = vec![Message::system("sys")];
    for _ in 0..10 {
        messages.push(tool_use_msg.clone());
        messages.push(tool_result_msg.clone());
    }
    messages.push(error_tool_result_msg);

    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 1);
    mw.process(&mut ctx).await.unwrap();

    assert!(
        ctx.messages.len() < original_len,
        "expected compaction with tool IO messages, from {} to {}",
        original_len,
        ctx.messages.len()
    );
}

#[tokio::test]
async fn smart_compact_with_long_tool_result_truncation() {
    let mw = SmartCompact::new(1);

    let long_content = "x".repeat(500);
    let tool_result_msg = Message {
        id: None,
        role: loopal_message::MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: long_content,
            is_error: false,
        }],
    };

    let mut messages = vec![Message::system("sys")];
    for _ in 0..5 {
        messages.push(Message::user(&"y".repeat(200)));
        messages.push(tool_result_msg.clone());
    }

    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 1);
    mw.process(&mut ctx).await.unwrap();

    assert!(
        ctx.messages.len() < original_len,
        "expected compaction, from {} to {}",
        original_len,
        ctx.messages.len()
    );
}

#[tokio::test]
async fn smart_compact_empty_old_messages_no_change() {
    let mw = SmartCompact::new(10);
    let messages = vec![large_message(400), large_message(400), large_message(400)];
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 50);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), original_len);
}
