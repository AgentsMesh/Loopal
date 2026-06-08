use super::*;
use loopal_turn::{
    AssistantOutput, ServerBlock, ServerToolCall, ServerToolPair, ServerToolResult, StopReason,
    TextBlock, ThinkingBlock, ToolCall, ToolCallId, TurnStep,
};

fn turn_with_user(content: &str) -> Turn {
    Turn::new(TurnTrigger::UserInput {
        envelope_id: String::new(),
        content: content.into(),
        images: Vec::new(),
    })
}

fn web_search_pair() -> ServerToolPair {
    ServerToolPair {
        call: ServerToolCall {
            id: "ws_1".into(),
            name: "web_search".into(),
            input: serde_json::json!({"query": "x"}),
        },
        result: ServerToolResult {
            block_type: "web_search_tool_result".into(),
            content: serde_json::json!({"status": "completed"}),
        },
    }
}

fn assistant_content_with(server_blocks: Vec<ServerBlock>) -> Vec<serde_json::Value> {
    let provider = AnthropicProvider::new(String::new());
    let mut turn = turn_with_user("q");
    turn.body.steps.push(TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks,
            stop_reason: StopReason::EndTurn,
        },
    });
    let params = ChatParams::new("claude-sonnet-4-20250514".into(), vec![turn], String::new());
    let out = provider.build_messages_json_from_turns(&params);
    out.into_iter()
        .find(|m| m["role"] == "assistant")
        .and_then(|m| m["content"].as_array().cloned())
        .expect("assistant message with content array")
}

// reason: 完整 turn(reasoning + text + tool_use)必须按 server→text→tool_use 投影,
// 覆盖 build_assistant 的 text_blocks / tool_calls 两个循环臂。
#[test]
fn full_turn_orders_reasoning_then_text_then_tool_use() {
    let provider = AnthropicProvider::new(String::new());
    let mut turn = turn_with_user("q");
    turn.body.steps.push(TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
            text_blocks: vec![TextBlock {
                text: "here".into(),
            }],
            tool_calls: vec![ToolCall {
                id: ToolCallId::new("tc_1"),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "a.rs"}),
            }],
            server_blocks: vec![ServerBlock::Reasoning(ThinkingBlock {
                thinking: "r".into(),
                signature: Some("sig".into()),
            })],
            stop_reason: StopReason::ToolUse,
        },
    });
    let params = ChatParams::new("claude-sonnet-4-20250514".into(), vec![turn], String::new());
    let out = provider.build_messages_json_from_turns(&params);
    let content = out
        .into_iter()
        .find(|m| m["role"] == "assistant")
        .and_then(|m| m["content"].as_array().cloned())
        .unwrap();
    let types: Vec<&str> = content.iter().filter_map(|b| b["type"].as_str()).collect();
    assert_eq!(types, vec!["thinking", "text", "tool_use"]);
}

#[test]
fn reasoning_emits_thinking_first_with_signature() {
    let content = assistant_content_with(vec![
        ServerBlock::Reasoning(ThinkingBlock {
            thinking: "deep".into(),
            signature: Some("sig-1".into()),
        }),
        ServerBlock::ToolPair(web_search_pair()),
    ]);
    assert_eq!(content[0]["type"], "thinking");
    assert_eq!(content[0]["signature"], "sig-1");
    assert_eq!(content[1]["type"], "server_tool_use");
    assert_eq!(content[2]["type"], "web_search_tool_result");
}

// reason: 无签名 Reasoning 必须被跳过——Anthropic 密码学校验签名，空签名会 400。
#[test]
fn reasoning_without_signature_is_skipped() {
    let content = assistant_content_with(vec![
        ServerBlock::Reasoning(ThinkingBlock {
            thinking: "unsigned".into(),
            signature: None,
        }),
        ServerBlock::ToolPair(web_search_pair()),
    ]);
    assert!(
        content.iter().all(|b| b["type"] != "thinking"),
        "unsigned reasoning must not be emitted: {content:?}"
    );
    assert_eq!(content[0]["type"], "server_tool_use");
}

// reason: 对抗——空字符串签名等同无签名(Anthropic 同样拒绝),必须走 filter 跳过臂。
#[test]
fn reasoning_with_empty_signature_is_skipped() {
    let content = assistant_content_with(vec![ServerBlock::Reasoning(ThinkingBlock {
        thinking: "blank sig".into(),
        signature: Some(String::new()),
    })]);
    assert!(
        content.iter().all(|b| b["type"] != "thinking"),
        "empty-string signature must be skipped like unsigned: {content:?}"
    );
}
