use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider::OpenAiProvider;
use loopal_provider_api::ChatParams;
use loopal_tool_api::ToolDefinition;
use serde_json::json;

fn make_provider() -> OpenAiProvider {
    OpenAiProvider::new("test-key".to_string())
}

fn make_params(
    messages: Vec<Message>,
    tools: Vec<ToolDefinition>,
    system_prompt: &str,
) -> ChatParams {
    ChatParams {
        model: "gpt-4o".to_string(),
        messages,
        system_prompt: system_prompt.to_string(),
        tools,
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        debug_dump_dir: None,
    }
}

#[test]
fn test_build_input_user_text() {
    let provider = make_provider();
    let params = make_params(vec![Message::user("Hello")], vec![], "");
    let input = provider.build_input(&params);
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[0]["content"][0]["type"], "input_text");
    assert_eq!(input[0]["content"][0]["text"], "Hello");
}

#[test]
fn test_build_input_tool_result_becomes_function_call_output() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_123".to_string(),
                content: "file contents here".to_string(),
                is_error: false,
            }],
        }],
        vec![],
        "",
    );
    let input = provider.build_input(&params);
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["type"], "function_call_output");
    assert_eq!(input[0]["call_id"], "call_123");
}

#[test]
fn test_build_input_assistant_with_tool_calls() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me check".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "call_abc".to_string(),
                    name: "read_file".to_string(),
                    input: json!({"path": "main.rs"}),
                },
            ],
        }],
        vec![],
        "",
    );
    let input = provider.build_input(&params);
    // First: assistant message with text, then function_call item
    assert_eq!(input.len(), 2);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "assistant");
    assert_eq!(input[1]["type"], "function_call");
    assert_eq!(input[1]["call_id"], "call_abc");
    assert_eq!(input[1]["name"], "read_file");
}

#[test]
fn test_build_tools_function_format() {
    let provider = make_provider();
    let params = make_params(
        vec![],
        vec![ToolDefinition {
            name: "bash".to_string(),
            description: "Run a shell command".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "command": {"type": "string"} },
                "required": ["command"]
            }),
        }],
        "",
    );
    let tools = provider.build_tools(&params);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["name"], "bash");
}

#[test]
fn test_build_tools_replaces_web_search() {
    let provider = make_provider();
    let params = make_params(
        vec![],
        vec![
            ToolDefinition {
                name: "WebSearch".to_string(),
                description: "Search the web".to_string(),
                input_schema: json!({}),
            },
            ToolDefinition {
                name: "bash".to_string(),
                description: "Run command".to_string(),
                input_schema: json!({}),
            },
        ],
        "",
    );
    let tools = provider.build_tools(&params);
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["type"], "web_search");
    assert_eq!(tools[1]["type"], "function");
    assert_eq!(tools[1]["name"], "bash");
}

#[test]
fn test_system_prompt_not_in_input() {
    let provider = make_provider();
    let params = make_params(vec![], vec![], "You are helpful");
    let input = provider.build_input(&params);
    // System prompt goes to `instructions`, not in input array
    assert!(input.is_empty());
}
