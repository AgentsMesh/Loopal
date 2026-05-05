use loopal_message::{ContentBlock, ImageSource, Message, MessageRole};
use loopal_provider::AnthropicProvider;
use loopal_provider_api::ChatParams;
use loopal_tool_api::ToolDefinition;
use serde_json::json;

fn make_provider() -> AnthropicProvider {
    AnthropicProvider::new("test-key".to_string())
}

fn make_params(messages: Vec<Message>, tools: Vec<ToolDefinition>) -> ChatParams {
    ChatParams {
        model: "claude-sonnet-4-20250514".to_string(),
        messages,
        system_prompt: String::new(),
        tools,
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

#[test]
fn test_build_messages_text_only() {
    let provider = make_provider();
    let params = make_params(
        vec![Message::user("Hello"), Message::assistant("Hi there")],
        vec![],
    );
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["role"], "user");
    assert_eq!(msgs[0]["content"][0]["type"], "text");
    assert_eq!(msgs[0]["content"][0]["text"], "Hello");
    assert_eq!(msgs[1]["role"], "assistant");
    assert_eq!(msgs[1]["content"][0]["text"], "Hi there");
}

#[test]
fn test_build_messages_with_tool_use() {
    let provider = make_provider();
    let params = make_params(
        vec![
            Message {
                id: None,
                role: MessageRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "tu_1".to_string(),
                    name: "bash".to_string(),
                    input: json!({"cmd": "ls"}),
                }],
            },
            Message {
                id: None,
                role: MessageRole::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "tu_1".to_string(),
                    content: "file.txt".to_string(),
                    is_error: false,
                    metadata: None,
                }],
            },
        ],
        vec![],
    );
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["content"][0]["type"], "tool_use");
    assert_eq!(msgs[0]["content"][0]["id"], "tu_1");
    assert_eq!(msgs[0]["content"][0]["name"], "bash");
    assert_eq!(msgs[0]["content"][0]["input"]["cmd"], "ls");
    assert_eq!(msgs[1]["content"][0]["type"], "tool_result");
    assert_eq!(msgs[1]["content"][0]["tool_use_id"], "tu_1");
    assert_eq!(msgs[1]["content"][0]["content"], "file.txt");
    assert_eq!(msgs[1]["content"][0]["is_error"], false);
}

#[test]
fn test_build_messages_filters_system() {
    let provider = make_provider();
    let params = make_params(
        vec![Message::system("You are helpful"), Message::user("Hello")],
        vec![],
    );
    let msgs = provider.build_messages(&params);
    // System messages should be filtered out
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["role"], "user");
}

#[test]
fn test_build_messages_with_image() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".to_string(),
                    media_type: "image/png".to_string(),
                    data: "iVBOR...".to_string(),
                },
            }],
        }],
        vec![],
    );
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["content"][0]["type"], "image");
    assert_eq!(msgs[0]["content"][0]["source"]["type"], "base64");
    assert_eq!(msgs[0]["content"][0]["source"]["media_type"], "image/png");
    assert_eq!(msgs[0]["content"][0]["source"]["data"], "iVBOR...");
}

#[test]
fn test_build_tools_empty() {
    let provider = make_provider();
    let params = make_params(vec![], vec![]);
    let tools = provider.build_tools(&params);
    assert!(tools.is_empty());
}

#[test]
fn test_build_tools_with_entries() {
    let provider = make_provider();
    let params = make_params(
        vec![],
        vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        }],
    );
    let tools = provider.build_tools(&params);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "read_file");
    assert_eq!(tools[0]["description"], "Read a file");
    assert_eq!(tools[0]["input_schema"]["type"], "object");
    assert_eq!(
        tools[0]["input_schema"]["properties"]["path"]["type"],
        "string"
    );
}
