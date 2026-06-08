use super::stream_helpers::test_chat_params;
use loopal_provider::OpenAiProvider;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use serde_json::json;

fn assistant(content: Vec<ContentBlock>) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content,
        origin: None,
        ephemeral_in_history: false,
    }
}

fn reasoning(id: &str) -> ContentBlock {
    ContentBlock::Thinking {
        thinking: format!("thinking {id}"),
        signature: Some(id.into()),
    }
}

fn web_search(id: &str) -> Vec<ContentBlock> {
    vec![
        ContentBlock::ServerToolUse {
            id: id.into(),
            name: "web_search".into(),
            input: json!({"query": "q"}),
        },
        ContentBlock::ServerToolResult {
            block_type: "web_search_tool_result".into(),
            tool_use_id: id.into(),
            content: json!({"status": "completed"}),
        },
    ]
}

fn item_kinds(input: &[serde_json::Value]) -> Vec<(String, String)> {
    input
        .iter()
        .filter_map(|v| {
            let ty = v["type"].as_str()?;
            if ty == "reasoning" || ty == "web_search_call" {
                Some((ty.to_string(), v["id"].as_str().unwrap_or("").to_string()))
            } else {
                None
            }
        })
        .collect()
}

// reason: 回归 #190 — 多个 web_search_call 在 wire `input` 里必须各自带前导 reasoning
// item，否则 OpenAI Responses API 400 "web_search_call provided without its
// required reasoning item"。这是当时线上 400 的精确复现。
#[test]
fn each_web_search_call_is_preceded_by_its_reasoning_item() {
    let mut content = vec![reasoning("rs_1")];
    content.extend(web_search("ws_1"));
    content.push(reasoning("rs_2"));
    content.extend(web_search("ws_2"));

    let provider = OpenAiProvider::new("test-key".into());
    let input = provider.build_input_from_messages(&[assistant(content)], &test_chat_params());

    assert_eq!(
        item_kinds(&input),
        vec![
            ("reasoning".into(), "rs_1".into()),
            ("web_search_call".into(), "ws_1".into()),
            ("reasoning".into(), "rs_2".into()),
            ("web_search_call".into(), "ws_2".into()),
        ],
        "every web_search_call must be immediately preceded by its reasoning item"
    );
}

#[test]
fn single_reasoning_web_search_round_trips() {
    let mut content = vec![reasoning("rs_a")];
    content.extend(web_search("ws_a"));

    let provider = OpenAiProvider::new("test-key".into());
    let input = provider.build_input_from_messages(&[assistant(content)], &test_chat_params());

    assert_eq!(
        item_kinds(&input),
        vec![
            ("reasoning".into(), "rs_a".into()),
            ("web_search_call".into(), "ws_a".into()),
        ]
    );
}
