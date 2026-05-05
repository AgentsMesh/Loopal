use loopal_message::{ContentBlock, ImageSource, Message, MessageRole};
use loopal_provider::GoogleProvider;
use loopal_provider_api::ChatParams;
use loopal_tool_api::ToolDefinition;
use serde_json::json;

fn make_provider() -> GoogleProvider {
    GoogleProvider::new("test-key".to_string())
}

fn make_params(messages: Vec<Message>, tools: Vec<ToolDefinition>) -> ChatParams {
    ChatParams {
        model: "gemini-2.0-flash".to_string(),
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
fn test_build_contents_text() {
    let provider = make_provider();
    let params = make_params(
        vec![Message::user("Hello"), Message::assistant("Hi there")],
        vec![],
    );
    let contents = provider.build_contents(&params);
    assert_eq!(contents.len(), 2);
    assert_eq!(contents[0]["role"], "user");
    assert_eq!(contents[0]["parts"][0]["text"], "Hello");
    assert_eq!(contents[1]["role"], "model");
    assert_eq!(contents[1]["parts"][0]["text"], "Hi there");
}

#[test]
fn test_build_contents_filters_system() {
    let provider = make_provider();
    let params = make_params(
        vec![
            Message::system("System instruction"),
            Message::user("Hello"),
        ],
        vec![],
    );
    let contents = provider.build_contents(&params);
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["role"], "user");
}

#[test]
fn test_build_contents_with_function_call() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                input: json!({"path": "main.rs"}),
            }],
        }],
        vec![],
    );
    let contents = provider.build_contents(&params);
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["role"], "model");
    let part = &contents[0]["parts"][0];
    assert_eq!(part["functionCall"]["name"], "read_file");
    assert_eq!(part["functionCall"]["args"]["path"], "main.rs");
}

#[test]
fn test_build_contents_with_function_response() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: "file contents".to_string(),
                is_error: false,
                metadata: None,
            }],
        }],
        vec![],
    );
    let contents = provider.build_contents(&params);
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["role"], "user");
    let part = &contents[0]["parts"][0];
    assert!(part.get("functionResponse").is_some());
    assert_eq!(
        part["functionResponse"]["response"]["result"],
        "file contents"
    );
}

#[test]
fn test_build_tools_empty() {
    let provider = make_provider();
    let params = make_params(vec![], vec![]);
    let tools = provider.build_tools(&params);
    assert!(tools.is_empty());
}

#[test]
fn test_build_tools_with_declarations() {
    let provider = make_provider();
    let params = make_params(
        vec![],
        vec![
            ToolDefinition {
                name: "bash".to_string(),
                description: "Run command".to_string(),
                input_schema: json!({"type": "object", "properties": {"cmd": {"type": "string"}}}),
            },
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            },
        ],
    );
    let tools = provider.build_tools(&params);
    assert_eq!(tools.len(), 1); // Wrapped in single functionDeclarations object
    let declarations = tools[0]["functionDeclarations"].as_array().unwrap();
    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0]["name"], "bash");
    assert_eq!(declarations[0]["description"], "Run command");
    assert_eq!(declarations[1]["name"], "read_file");
    assert_eq!(declarations[1]["description"], "Read a file");
}

#[test]
fn test_build_contents_with_image() {
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
    let contents = provider.build_contents(&params);
    assert_eq!(contents.len(), 1);
    let part = &contents[0]["parts"][0];
    assert_eq!(part["inlineData"]["mimeType"], "image/png");
    assert_eq!(part["inlineData"]["data"], "iVBOR...");
}
