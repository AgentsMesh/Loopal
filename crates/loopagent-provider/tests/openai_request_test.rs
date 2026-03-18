use loopagent_provider::OpenAiProvider;
use loopagent_types::message::{ContentBlock, Message, MessageRole};
use loopagent_types::provider::ChatParams;
use loopagent_types::tool::ToolDefinition;
use serde_json::{json, Value};

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
        debug_dump_dir: None,
    }
}

#[test]
fn test_build_messages_includes_system_prompt() {
    let provider = make_provider();
    let params = make_params(vec![], vec![], "You are helpful");
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[0]["content"], "You are helpful");
}

#[test]
fn test_build_messages_user_text() {
    let provider = make_provider();
    let params = make_params(
        vec![Message::user("Hello"), Message::user("World")],
        vec![],
        "",
    );
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["role"], "user");
    assert_eq!(msgs[0]["content"], "Hello");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "World");
}

#[test]
fn test_build_messages_tool_result_becomes_tool_role() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
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
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["role"], "tool");
    assert_eq!(msgs[0]["tool_call_id"], "call_123");
    assert_eq!(msgs[0]["content"], "file contents here");
}

#[test]
fn test_build_messages_assistant_with_tool_calls() {
    let provider = make_provider();
    let params = make_params(
        vec![Message {
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
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["role"], "assistant");
    assert_eq!(msgs[0]["content"], "Let me check");
    let tool_calls = msgs[0]["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["id"], "call_abc");
    assert_eq!(tool_calls[0]["type"], "function");
    assert_eq!(tool_calls[0]["function"]["name"], "read_file");
    // arguments is stringified JSON
    let args_str = tool_calls[0]["function"]["arguments"].as_str().unwrap();
    let args: Value = serde_json::from_str(args_str).unwrap();
    assert_eq!(args["path"], "main.rs");
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
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }),
        }],
        "",
    );
    let tools = provider.build_tools(&params);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "bash");
    assert_eq!(tools[0]["function"]["description"], "Run a shell command");
    assert_eq!(tools[0]["function"]["parameters"]["type"], "object");
}
