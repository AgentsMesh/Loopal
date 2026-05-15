use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider::OpenAiProvider;
use loopal_provider_api::ChatParams;
use serde_json::json;

fn make_provider() -> OpenAiProvider {
    OpenAiProvider::new("test-key".to_string())
}

fn make_params(messages: Vec<Message>, system_prompt: &str) -> ChatParams {
    ChatParams {
        model: "gpt-5.5".to_string(),
        messages,
        system_prompt: system_prompt.to_string(),
        tools: vec![],
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

#[test]
fn thinking_becomes_reasoning_item() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "Let me search for that.".to_string(),
                    signature: Some("rs_abc123".to_string()),
                },
                ContentBlock::ServerToolUse {
                    id: "ws_def456".to_string(),
                    name: "web_search".to_string(),
                    input: json!({"query": "rust async"}),
                },
            ],
        }],
        "",
    );
    let input = provider.build_input(&params);
    assert_eq!(input.len(), 2);
    assert_eq!(input[0]["type"], "reasoning");
    assert_eq!(input[0]["id"], "rs_abc123");
    assert_eq!(input[0]["summary"][0]["type"], "summary_text");
    assert_eq!(input[0]["summary"][0]["text"], "Let me search for that.");
    assert_eq!(input[1]["type"], "web_search_call");
    assert_eq!(input[1]["id"], "ws_def456");
}

#[test]
fn thinking_without_signature_skipped() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "some thought".to_string(),
                    signature: None,
                },
                ContentBlock::Text {
                    text: "Hello".to_string(),
                },
            ],
        }],
        "",
    );
    let input = provider.build_input(&params);
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "assistant");
}

#[test]
fn thinking_with_empty_signature_skipped() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "some thought".to_string(),
                    signature: Some(String::new()),
                },
                ContentBlock::Text {
                    text: "Hello".to_string(),
                },
            ],
        }],
        "",
    );
    let input = provider.build_input(&params);
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["type"], "message");
}

#[test]
fn multi_turn_reasoning_before_web_search() {
    let provider = make_provider();
    let params = make_params(
        vec![
            Message {
                id: None,
                role: MessageRole::Assistant,
                content: vec![
                    ContentBlock::Thinking {
                        thinking: "First search".to_string(),
                        signature: Some("rs_001".to_string()),
                    },
                    ContentBlock::ServerToolUse {
                        id: "ws_001".to_string(),
                        name: "web_search".to_string(),
                        input: json!({"query": "first query"}),
                    },
                    ContentBlock::Text {
                        text: "Found it.".to_string(),
                    },
                ],
            },
            Message::user("Tell me more"),
            Message {
                id: None,
                role: MessageRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: "Sure, here's more.".to_string(),
                }],
            },
        ],
        "",
    );
    let input = provider.build_input(&params);
    assert_eq!(input[0]["type"], "reasoning");
    assert_eq!(input[0]["id"], "rs_001");
    assert_eq!(input[1]["type"], "web_search_call");
    assert_eq!(input[1]["id"], "ws_001");
    assert_eq!(input[2]["type"], "message");
    assert_eq!(input[2]["role"], "assistant");
    assert_eq!(input[3]["type"], "message");
    assert_eq!(input[3]["role"], "user");
    assert_eq!(input[4]["type"], "message");
    assert_eq!(input[4]["role"], "assistant");
}
