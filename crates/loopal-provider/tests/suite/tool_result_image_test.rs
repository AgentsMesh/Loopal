use loopal_provider::{AnthropicProvider, GoogleProvider, OpenAiCompatProvider, OpenAiProvider};
use loopal_provider_api::ChatParams;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_tool_api::ToolDefinition;
use loopal_tool_invocation::ToolImageBlock;

fn params_with(messages: Vec<Message>) -> ChatParams {
    ChatParams {
        model: "test-model".to_string(),
        messages,
        turns: vec![],
        system_prompt: String::new(),
        tools: Vec::<ToolDefinition>::new(),
        max_tokens: 4096,
        temperature: None,
        thinking: None,
        continuation_intent: None,
        debug_dump_dir: None,
    }
}

fn tool_result_with_image(tool_use_id: &str, content: &str, data: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: content.to_string(),
            images: vec![ToolImageBlock::Inline {
                media_type: "image/png".to_string(),
                data: data.to_string(),
            }],
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }
}

#[test]
fn anthropic_tool_result_with_image_inlines_array() {
    let provider = AnthropicProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_image("tu_1", "Loaded.", "AAAA")]);
    let msgs = provider.build_messages(&params);
    let block = &msgs[0]["content"][0];
    assert_eq!(block["type"], "tool_result");
    assert_eq!(block["tool_use_id"], "tu_1");
    assert!(
        block["content"].is_array(),
        "content must be array when images present"
    );
    let arr = block["content"].as_array().unwrap();
    assert_eq!(arr[0]["type"], "text");
    assert_eq!(arr[0]["text"], "Loaded.");
    assert_eq!(arr[1]["type"], "image");
    assert_eq!(arr[1]["source"]["type"], "base64");
    assert_eq!(arr[1]["source"]["media_type"], "image/png");
    assert_eq!(arr[1]["source"]["data"], "AAAA");
}

#[test]
fn anthropic_tool_result_text_only_stays_string() {
    let provider = AnthropicProvider::new("k".to_string());
    let params = params_with(vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu_1".to_string(),
            content: "ok".to_string(),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }]);
    let msgs = provider.build_messages(&params);
    assert_eq!(msgs[0]["content"][0]["content"], "ok");
}

#[test]
fn openai_tool_result_with_image_splits_to_separate_message() {
    let provider = OpenAiProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_image("call_1", "Loaded.", "BBBB")]);
    let input = provider.build_input(&params);
    let fco = input
        .iter()
        .find(|v| v["type"] == "function_call_output")
        .expect("function_call_output present");
    assert_eq!(fco["call_id"], "call_1");
    assert_eq!(fco["output"], "Loaded.");
    let img_msg = input
        .iter()
        .find(|v| v["type"] == "message" && v["role"] == "user")
        .expect("user image message present");
    assert_eq!(img_msg["content"][0]["type"], "input_image");
    let url = img_msg["content"][0]["image_url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,BBBB"));
}

#[test]
fn google_tool_result_with_image_appends_inline_part() {
    let provider = GoogleProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_image("tu_1", "Loaded.", "CCCC")]);
    let contents = provider.build_contents(&params);
    let parts = contents[0]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts[0]["functionResponse"]["response"]["result"],
        "Loaded."
    );
    assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    assert_eq!(parts[1]["inlineData"]["data"], "CCCC");
}

#[test]
fn openai_image_only_uses_placeholder() {
    let provider = OpenAiProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_image("call_1", "", "DDDD")]);
    let input = provider.build_input(&params);
    let fco = input
        .iter()
        .find(|v| v["type"] == "function_call_output")
        .unwrap();
    assert_eq!(fco["output"], "[Image attached]");
}

fn tool_result_with_session_resource(tool_use_id: &str, content: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: content.to_string(),
            images: vec![ToolImageBlock::session_resource(
                "unhydrated-id",
                "image/png",
                65536,
            )],
            is_error: false,
            metadata: None,
        }],
        origin: None,
        ephemeral_in_history: false,
    }
}

#[test]
fn anthropic_drops_unhydrated_session_resource_silently() {
    let provider = AnthropicProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_session_resource("tu_1", "Loaded.")]);
    let msgs = provider.build_messages(&params);
    let block = &msgs[0]["content"][0];
    assert_eq!(block["type"], "tool_result");
    if let Some(arr) = block["content"].as_array() {
        assert!(
            arr.iter().all(|b| b["type"] != "image"),
            "unhydrated SessionResource must not produce an image block: {arr:?}"
        );
    }
    // String content fallback is also acceptable when only text remains.
}

#[test]
fn openai_drops_unhydrated_session_resource_silently() {
    let provider = OpenAiProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_session_resource("call_1", "Loaded.")]);
    let input = provider.build_input(&params);
    let any_image = input
        .iter()
        .any(|v| v["content"][0]["type"] == "input_image");
    assert!(
        !any_image,
        "unhydrated SessionResource must not produce input_image item"
    );
}

#[test]
fn google_drops_unhydrated_session_resource_silently() {
    let provider = GoogleProvider::new("k".to_string());
    let params = params_with(vec![tool_result_with_session_resource("tu_1", "Loaded.")]);
    let contents = provider.build_contents(&params);
    let any_inline = contents[0]["parts"]
        .as_array()
        .map(|parts| parts.iter().any(|p| p.get("inlineData").is_some()))
        .unwrap_or(false);
    assert!(
        !any_inline,
        "unhydrated SessionResource must not produce inlineData part"
    );
}

#[test]
fn openai_compat_drops_unhydrated_session_resource_silently() {
    let provider = OpenAiCompatProvider::new(
        "k".to_string(),
        "https://example.com".to_string(),
        "test".to_string(),
    );
    let params = params_with(vec![tool_result_with_session_resource("call_1", "Loaded.")]);
    let msgs = provider.build_messages(&params);
    let any_image = msgs.iter().any(|m| m["content"][0]["type"] == "image_url");
    assert!(
        !any_image,
        "unhydrated SessionResource must not produce image_url part"
    );
}
