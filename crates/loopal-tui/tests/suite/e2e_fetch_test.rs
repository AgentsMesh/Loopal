//! E2E tests for Fetch (via wiremock) and Bash timeout.

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
async fn test_bash_timeout() {
    // Clear store before and after to avoid polluting other tests.
    loopal_tool_background::clear_store();
    // Bash with timeout=0 (0 seconds → 0ms) and a command that sleeps 60s.
    // The streaming path converts timeout to a background task (success, not error).
    let calls = vec![
        chunks::tool_turn(
            "tc-to",
            "Bash",
            serde_json::json!({"command": "sleep 60", "timeout": 0}),
        ),
        chunks::text_turn("Timed out."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    // Timeout now converts to background → success ToolResult (not error)
    assertions::assert_has_tool_result(&evts, "Bash", false);

    let results: Vec<&str> = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolResult { name, result, .. }
                if name == "Bash" && result.to_lowercase().contains("timed out") =>
            {
                Some(result.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        !results.is_empty(),
        "bash timeout-to-background result should mention 'timed out'"
    );
    assert!(
        results[0].contains("process_id"),
        "should include background process_id"
    );

    // Clean up background tasks created by this test
    loopal_tool_background::clear_store();
}
