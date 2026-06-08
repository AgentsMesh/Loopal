use loopal_provider_api::{ContentBlock, MessageRole};

use super::make_runner;

#[test]
fn test_record_assistant_message_text_only() {
    let (mut runner, _rx) = make_runner();
    assert!(runner.turns.view().is_empty());

    runner.record_assistant_message("Hello, world!", &[], vec![]);

    assert_eq!(runner.turns.view().len(), 1);
    let msg = &runner.turns.view().messages()[0];
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.content.len(), 1);
    match &msg.content[0] {
        ContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
        other => panic!("expected Text block, got {other:?}"),
    }
}

#[test]
fn test_record_assistant_message_with_tool_uses() {
    let (mut runner, _rx) = make_runner();

    let tool_uses = vec![
        (
            "tc-1".to_string(),
            "bash".to_string(),
            serde_json::json!({"command": "ls"}),
        ),
        (
            "tc-2".to_string(),
            "read".to_string(),
            serde_json::json!({"file": "test.rs"}),
        ),
    ];

    runner.record_assistant_message("Let me check that.", &tool_uses, vec![]);

    assert_eq!(runner.turns.view().len(), 1);
    let msg = &runner.turns.view().messages()[0];
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.content.len(), 3);

    match &msg.content[0] {
        ContentBlock::Text { text } => assert_eq!(text, "Let me check that."),
        other => panic!("expected Text, got {other:?}"),
    }
    match &msg.content[1] {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-1");
            assert_eq!(name, "bash");
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
    match &msg.content[2] {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-2");
            assert_eq!(name, "read");
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[test]
fn test_record_assistant_message_empty_adds_nothing() {
    let (mut runner, _rx) = make_runner();
    runner.record_assistant_message("", &[], vec![]);

    assert!(
        runner.turns.view().is_empty(),
        "empty content should not produce a message"
    );
}

#[test]
fn test_record_assistant_message_tool_uses_only_no_text() {
    let (mut runner, _rx) = make_runner();

    let tool_uses = vec![(
        "tc-1".to_string(),
        "bash".to_string(),
        serde_json::json!({"command": "echo hi"}),
    )];

    runner.record_assistant_message("", &tool_uses, vec![]);

    assert_eq!(runner.turns.view().len(), 1);
    let msg = &runner.turns.view().messages()[0];
    assert_eq!(msg.content.len(), 1);
    match &msg.content[0] {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-1");
            assert_eq!(name, "bash");
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[tokio::test]
async fn test_record_assistant_message_saves_to_session() {
    let (mut runner, _rx) = make_runner();
    runner.record_assistant_message("test message", &[], vec![]);
    assert_eq!(runner.turns.view().len(), 1);
    assert_eq!(
        runner.turns.view().messages()[0].text_content(),
        "test message"
    );
}

// reason: 回归 #190 — 多个 reasoning + web_search 交错必须保序投影，每个
// web_search_call 前都有其 reasoning item，否则 OpenAI Responses API 400。
#[test]
fn record_interleaved_reasoning_and_web_search_preserves_pairing() {
    let (mut runner, _rx) = make_runner();
    let server_blocks = vec![
        ContentBlock::Thinking {
            thinking: "r1".into(),
            signature: Some("rs_1".into()),
        },
        ContentBlock::ServerToolUse {
            id: "ws_1".into(),
            name: "web_search".into(),
            input: serde_json::json!({"query": "a"}),
        },
        ContentBlock::ServerToolResult {
            block_type: "web_search_tool_result".into(),
            tool_use_id: "ws_1".into(),
            content: serde_json::json!({"status": "completed"}),
        },
        ContentBlock::Thinking {
            thinking: "r2".into(),
            signature: Some("rs_2".into()),
        },
        ContentBlock::ServerToolUse {
            id: "ws_2".into(),
            name: "web_search".into(),
            input: serde_json::json!({"query": "b"}),
        },
        ContentBlock::ServerToolResult {
            block_type: "web_search_tool_result".into(),
            tool_use_id: "ws_2".into(),
            content: serde_json::json!({"status": "completed"}),
        },
    ];
    runner.record_assistant_message("done", &[], server_blocks);

    let msg = &runner.turns.view().messages()[0];
    let kinds: Vec<&str> = msg
        .content
        .iter()
        .map(|b| match b {
            ContentBlock::Thinking { signature, .. } => match signature.as_deref() {
                Some("rs_1") => "think:rs_1",
                Some("rs_2") => "think:rs_2",
                _ => "think:?",
            },
            ContentBlock::ServerToolUse { id, .. } => match id.as_str() {
                "ws_1" => "use:ws_1",
                "ws_2" => "use:ws_2",
                _ => "use:?",
            },
            ContentBlock::ServerToolResult { tool_use_id, .. } => match tool_use_id.as_str() {
                "ws_1" => "res:ws_1",
                "ws_2" => "res:ws_2",
                _ => "res:?",
            },
            ContentBlock::Text { .. } => "text",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "think:rs_1",
            "use:ws_1",
            "res:ws_1",
            "think:rs_2",
            "use:ws_2",
            "res:ws_2",
            "text",
        ],
        "each web_search_call must be preceded by its reasoning, in stream order"
    );
}
