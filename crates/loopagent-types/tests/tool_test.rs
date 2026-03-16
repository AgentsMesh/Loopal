use loopagent_types::tool::ToolResult;

#[test]
fn test_tool_result_success() {
    let r = ToolResult::success("ok");
    assert_eq!(r.content, "ok");
    assert!(!r.is_error);
}

#[test]
fn test_tool_result_error() {
    let r = ToolResult::error("fail");
    assert_eq!(r.content, "fail");
    assert!(r.is_error);
}

#[test]
fn test_tool_result_success_from_string() {
    let r = ToolResult::success(String::from("hello"));
    assert_eq!(r.content, "hello");
    assert!(!r.is_error);
}
