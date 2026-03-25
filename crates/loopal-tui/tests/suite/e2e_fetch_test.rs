//! E2E tests for Fetch (via wiremock), WebSearch (missing API key), and Bash timeout.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::{assertions, chunks};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_fetch_via_mock_server() {
    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello from mock"))
        .mount(&mock_server)
        .await;

    let url = mock_server.uri();
    let calls = vec![
        chunks::tool_turn("tc-f", "Fetch", serde_json::json!({"url": url})),
        chunks::text_turn("Fetched the page."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_call(&evts, "Fetch");
    assertions::assert_has_tool_result(&evts, "Fetch", false);

    // Verify the result mentions the download or contains the body
    let results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "Fetch" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        results
            .iter()
            .any(|r| r.contains("Downloaded to:") || r.contains("Hello from mock")),
        "fetch result should contain download path or body, got: {results:?}"
    );
}

#[tokio::test]
async fn test_fetch_with_prompt() {
    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>Mock page content</body></html>")
                .insert_header("content-type", "text/html"),
        )
        .mount(&mock_server)
        .await;

    let url = mock_server.uri();
    let calls = vec![
        chunks::tool_turn(
            "tc-fp",
            "Fetch",
            serde_json::json!({"url": url, "prompt": "summarize"}),
        ),
        chunks::text_turn("Fetched with prompt."),
    ];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&evts, "Fetch", false);

    let results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. } if name == "Fetch" => {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    // With prompt, content is returned inline (converted from HTML)
    assert!(
        results.iter().any(|r| r.contains("Mock page content")),
        "fetch+prompt result should contain inline content, got: {results:?}"
    );
}

#[tokio::test]
async fn test_web_search_no_api_key() {
    // WebSearch requires TAVILY_API_KEY — without it, should return an error
    let calls = vec![
        chunks::tool_turn("tc-ws", "WebSearch", serde_json::json!({"query": "test"})),
        chunks::text_turn("Search failed."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    // Missing API key → tool returns error
    assertions::assert_has_tool_result(&evts, "WebSearch", true);
}

#[tokio::test]
async fn test_bash_timeout() {
    // Bash with a tiny timeout (100ms) and a command that sleeps 60s
    let calls = vec![
        chunks::tool_turn(
            "tc-to",
            "Bash",
            serde_json::json!({"command": "sleep 60", "timeout": 100}),
        ),
        chunks::text_turn("Timed out."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    // Timeout propagates as an error ToolResult
    assertions::assert_has_tool_result(&evts, "Bash", true);

    let results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. }
                if name == "Bash" && result.to_lowercase().contains("timeout") =>
            {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        !results.is_empty(),
        "bash timeout error should mention 'timeout'"
    );
}
