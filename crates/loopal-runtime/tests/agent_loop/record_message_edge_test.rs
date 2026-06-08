use loopal_provider_api::ContentBlock;

use super::make_runner;

fn count_server_use(content: &[ContentBlock]) -> usize {
    content
        .iter()
        .filter(|b| matches!(b, ContentBlock::ServerToolUse { .. }))
        .count()
}

// reason: 截断响应会留下没有 result 的 ServerToolUse。build_server_blocks 必须丢弃它
// (不配对成 ToolPair、不 panic),否则 wire 里出现孤立 web_search_call → API 400。
#[test]
fn orphan_server_tool_use_without_result_is_dropped() {
    let (mut runner, _rx) = make_runner();
    let server_blocks = vec![
        ContentBlock::Thinking {
            thinking: "r1".into(),
            signature: Some("rs_1".into()),
        },
        ContentBlock::ServerToolUse {
            id: "ws_orphan".into(),
            name: "web_search".into(),
            input: serde_json::json!({"query": "x"}),
        },
    ];
    runner.record_assistant_message("done", &[], server_blocks);

    let msg = &runner.turns.view().messages()[0];
    assert_eq!(
        count_server_use(&msg.content),
        0,
        "orphan web_search_call (no result) must be dropped"
    );
    assert!(
        msg.content
            .iter()
            .any(|b| matches!(b, ContentBlock::Thinking { .. })),
        "reasoning before the orphan must still survive"
    );
}

// reason: 孤立 ServerToolResult(无配对 use)不得凭空产出块。
#[test]
fn orphan_server_tool_result_without_use_is_ignored() {
    let (mut runner, _rx) = make_runner();
    let server_blocks = vec![ContentBlock::ServerToolResult {
        block_type: "web_search_tool_result".into(),
        tool_use_id: "ws_ghost".into(),
        content: serde_json::json!({"status": "completed"}),
    }];
    runner.record_assistant_message("hi", &[], server_blocks);

    let msg = &runner.turns.view().messages()[0];
    let has_server = msg.content.iter().any(|b| {
        matches!(
            b,
            ContentBlock::ServerToolUse { .. } | ContentBlock::ServerToolResult { .. }
        )
    });
    assert!(!has_server, "orphan result must not emit any server block");
}

// reason: 仅有 reasoning(无 text/tool)也必须被记录——has_server 分支,
// 否则纯思考转 web_search 的 turn 会被空响应 guard 误丢。
#[test]
fn reasoning_only_response_is_recorded() {
    let (mut runner, _rx) = make_runner();
    let server_blocks = vec![ContentBlock::Thinking {
        thinking: "only thinking".into(),
        signature: Some("rs_solo".into()),
    }];
    runner.record_assistant_message("", &[], server_blocks);

    assert_eq!(runner.turns.view().len(), 1, "reasoning-only must record");
    let msg = &runner.turns.view().messages()[0];
    assert!(matches!(msg.content[0], ContentBlock::Thinking { .. }));
}
