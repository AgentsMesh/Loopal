use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_fetch::{FetchParams, FetchTool};

use super::fetch_failure_backend::{FailureBackend, FailurePoint};

fn context(failure: FailurePoint) -> ToolContext {
    ToolContext::new(FailureBackend::new(failure), "fetch-failure-test")
}

fn tool() -> TypedBridge<FetchTool, FetchParams> {
    TypedBridge::new(FetchTool)
}

#[tokio::test]
async fn download_reports_each_storage_failure() {
    for (failure, expected) in [
        (FailurePoint::ResolveDirectory, "Failed to resolve temp dir"),
        (FailurePoint::CreateDirectory, "Failed to create temp dir"),
        (FailurePoint::ResolveFile, "Failed to resolve temp file"),
        (FailurePoint::WriteFile, "Failed to write temp file"),
    ] {
        let error = tool()
            .execute(
                serde_json::json!({"url": "https://source.example/data"}),
                &context(failure),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[tokio::test]
async fn successful_download_reports_cross_origin_redirect() {
    let result = tool()
        .execute(
            serde_json::json!({"url": "https://source.example/data"}),
            &context(FailurePoint::None),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(
        result
            .content
            .starts_with("Final-URL: https://redirect.example/final\n\nDownloaded to:")
    );
    assert!(result.content.contains("Content-Type: text/plain"));
    assert!(result.content.contains("Size: 7 bytes"));
}
