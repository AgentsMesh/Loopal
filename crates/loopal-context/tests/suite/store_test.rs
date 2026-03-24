use loopal_context::budget::ContextBudget;
use loopal_context::store::ContextStore;
use loopal_message::{ContentBlock, Message, MessageRole};

fn make_budget(message_budget: u32) -> ContextBudget {
    ContextBudget {
        context_window: message_budget * 2,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 0,
        safety_margin: 0,
        message_budget,
    }
}

fn big_tool_result_msg(size: usize) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "x".repeat(size),
            is_error: false,
        }],
    }
}

fn assistant_with_server_blocks(text: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::ServerToolUse {
                id: "st-1".into(),
                name: "web_search".into(),
                input: serde_json::json!({}),
            },
            ContentBlock::ServerToolResult {
                block_type: "web_search_tool_result".into(),
                tool_use_id: "st-1".into(),
                content: serde_json::json!({"results": "x".repeat(10_000)}),
            },
            ContentBlock::Text {
                text: text.to_string(),
            },
        ],
    }
}

#[test]
fn push_tool_results_caps_oversized() {
    let budget = make_budget(10_000);
    let mut store = ContextStore::new(budget);

    store.push_assistant(Message::assistant("do read"));
    // budget/8 = 1250 tokens, ~5000 chars. 100K chars = way over.
    store.push_tool_results(big_tool_result_msg(100_000));

    let msg = &store.messages()[1];
    if let ContentBlock::ToolResult { content, .. } = &msg.content[0] {
        assert!(content.len() < 100_000, "should be capped at ingestion");
    }
}

#[test]
fn push_assistant_strips_old_server_blocks() {
    let budget = make_budget(100_000);
    let mut store = ContextStore::new(budget);

    // First assistant with server blocks
    store.push_assistant(assistant_with_server_blocks("first results"));
    store.push_user(Message::user("thanks"));
    // Second assistant — should trigger stripping of first's server blocks
    store.push_assistant(Message::assistant("second response"));

    let first = &store.messages()[0];
    assert!(
        !first
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ServerToolResult { .. })),
        "old ServerToolResult should be condensed"
    );
}

#[test]
fn prepare_for_llm_preserves_last_thinking() {
    let budget = make_budget(100_000);
    let mut store = ContextStore::new(budget);

    store.push_assistant(Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Thinking {
                thinking: "old thought".into(),
                signature: Some("sig1".into()),
            },
            ContentBlock::Text {
                text: "first".into(),
            },
        ],
    });
    store.push_user(Message::user("next"));
    store.push_assistant(Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Thinking {
                thinking: "new thought".into(),
                signature: Some("sig2".into()),
            },
            ContentBlock::Text {
                text: "second".into(),
            },
        ],
    });

    let prepared = store.prepare_for_llm();
    // Old assistant should have thinking stripped
    let old_has_thinking = prepared[0]
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::Thinking { .. }));
    assert!(!old_has_thinking, "old thinking should be stripped");

    // Last assistant should keep thinking
    let new_has_thinking = prepared[2]
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::Thinking { .. }));
    assert!(new_has_thinking, "last thinking should be preserved");
}

#[test]
fn token_aware_keep_count_dynamic() {
    let budget = make_budget(10_000);
    let mut store = ContextStore::new(budget);

    // Push several small messages — all should fit in half budget
    for i in 0..20 {
        if i % 2 == 0 {
            store.push_assistant(Message::assistant(&format!("response {i}")));
        } else {
            store.push_user(Message::user(&format!("question {i}")));
        }
    }

    let keep = store.token_aware_keep_count();
    assert!(keep >= 2, "should keep at least 2");
    assert!(keep <= store.len(), "should not exceed total messages");
}

#[test]
fn from_messages_normalizes() {
    let messages = vec![
        assistant_with_server_blocks("old"),
        Message::user("mid"),
        Message::assistant("recent"),
    ];

    let budget = make_budget(100_000);
    let store = ContextStore::from_messages(messages, budget);

    // Old assistant's server blocks should be condensed after from_messages
    let first = &store.messages()[0];
    assert!(
        !first
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ServerToolResult { .. })),
        "from_messages should normalize old server blocks"
    );
}

#[test]
fn clear_empties_store() {
    let budget = make_budget(100_000);
    let mut store = ContextStore::new(budget);
    store.push_user(Message::user("hello"));
    store.clear();
    assert!(store.is_empty());
}

#[test]
fn truncate_works() {
    let budget = make_budget(100_000);
    let mut store = ContextStore::new(budget);
    store.push_assistant(Message::assistant("a"));
    store.push_user(Message::user("b"));
    store.push_assistant(Message::assistant("c"));
    store.truncate(1);
    assert_eq!(store.len(), 1);
}
