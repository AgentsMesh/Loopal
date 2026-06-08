use loopal_turn::{
    AssistantOutput, ServerBlock, ServerToolCall, ServerToolPair, ServerToolResult, StopReason,
    ThinkingBlock,
};

fn tool_pair() -> ServerToolPair {
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

#[test]
fn reasoning_block_round_trips() {
    let block = ServerBlock::Reasoning(ThinkingBlock {
        thinking: "deep".into(),
        signature: Some("rs_1".into()),
    });
    let json = serde_json::to_string(&block).unwrap();
    let back: ServerBlock = serde_json::from_str(&json).unwrap();
    match back {
        ServerBlock::Reasoning(t) => {
            assert_eq!(t.thinking, "deep");
            assert_eq!(t.signature.as_deref(), Some("rs_1"));
        }
        ServerBlock::ToolPair(_) => panic!("expected Reasoning"),
    }
}

#[test]
fn tool_pair_block_round_trips() {
    let block = ServerBlock::ToolPair(tool_pair());
    let json = serde_json::to_string(&block).unwrap();
    let back: ServerBlock = serde_json::from_str(&json).unwrap();
    match back {
        ServerBlock::ToolPair(p) => {
            assert_eq!(p.call.id, "ws_1");
            assert_eq!(p.result.block_type, "web_search_tool_result");
        }
        ServerBlock::Reasoning(_) => panic!("expected ToolPair"),
    }
}

// reason: 旧 turns.jsonl 的 AssistantOutput 携带顶层 `thinking` 字段且 server_blocks
// 是裸 `{call,result}`。新模型删了 thinking、server_blocks 变 Vec<ServerBlock>。
// 自定义 Deser 必须把旧 `{call,result}` 读成 ToolPair，并静默忽略残留 `thinking` key。
#[test]
fn deserializes_legacy_assistant_output_with_stray_thinking_key() {
    let legacy = r#"{
        "thinking": {"thinking": "old reasoning", "signature": "rs_old"},
        "text_blocks": [{"text": "hi"}],
        "tool_calls": [],
        "server_blocks": [
            {"call": {"id": "ws_1", "name": "web_search", "input": {}},
             "result": {"block_type": "web_search_tool_result", "content": {}}}
        ],
        "stop_reason": "EndTurn"
    }"#;
    let out: AssistantOutput = serde_json::from_str(legacy).unwrap();
    assert_eq!(out.text_blocks.len(), 1);
    assert_eq!(out.server_blocks.len(), 1);
    match &out.server_blocks[0] {
        ServerBlock::ToolPair(p) => assert_eq!(p.call.id, "ws_1"),
        ServerBlock::Reasoning(_) => panic!("legacy {{call,result}} must read as ToolPair"),
    }
}

#[test]
fn rejects_block_without_discriminating_key() {
    let bad = r#"{"unknown": 1}"#;
    let err = serde_json::from_str::<ServerBlock>(bad).unwrap_err();
    assert!(
        err.to_string().contains("ToolPair") || err.to_string().contains("Reasoning"),
        "error should name the expected variants: {err}"
    );
}

// reason: `call` 键存在但结构损坏(缺 result)→ from_value 失败,必须走 map_err
// 错误臂返回 Err 而非 panic 或误判。
#[test]
fn malformed_tool_pair_with_call_key_errors() {
    let bad = r#"{"call": {"id": "x", "name": "web_search", "input": {}}}"#;
    assert!(
        serde_json::from_str::<ServerBlock>(bad).is_err(),
        "call-keyed object missing `result` must error, not silently drop"
    );
}

// reason: `thinking` 键存在但类型错误(非字符串)→ Reasoning 错误臂返回 Err。
#[test]
fn malformed_reasoning_with_thinking_key_errors() {
    let bad = r#"{"thinking": 123}"#;
    assert!(
        serde_json::from_str::<ServerBlock>(bad).is_err(),
        "thinking-keyed object with non-string thinking must error"
    );
}

// reason: 对抗——同时含 `call` 与 `thinking` 键时,precedence 必须确定(call 先判),
// 否则旧 ToolPair 数据可能被误读成 Reasoning。
#[test]
fn both_keys_present_resolves_to_tool_pair() {
    let ambiguous = r#"{
        "call": {"id": "ws_x", "name": "web_search", "input": {}},
        "result": {"block_type": "web_search_tool_result", "content": {}},
        "thinking": "should be ignored"
    }"#;
    match serde_json::from_str::<ServerBlock>(ambiguous).unwrap() {
        ServerBlock::ToolPair(p) => assert_eq!(p.call.id, "ws_x"),
        ServerBlock::Reasoning(_) => panic!("`call` key must win over `thinking`"),
    }
}

#[test]
fn mixed_server_blocks_preserve_order() {
    let out = AssistantOutput {
        text_blocks: vec![],
        tool_calls: vec![],
        server_blocks: vec![
            ServerBlock::Reasoning(ThinkingBlock {
                thinking: "r1".into(),
                signature: Some("rs_1".into()),
            }),
            ServerBlock::ToolPair(tool_pair()),
        ],
        stop_reason: StopReason::EndTurn,
    };
    let json = serde_json::to_string(&out).unwrap();
    let back: AssistantOutput = serde_json::from_str(&json).unwrap();
    assert!(matches!(back.server_blocks[0], ServerBlock::Reasoning(_)));
    assert!(matches!(back.server_blocks[1], ServerBlock::ToolPair(_)));
}
