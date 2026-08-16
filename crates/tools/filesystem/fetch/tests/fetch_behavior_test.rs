use tokio::io::{AsyncReadExt, AsyncWriteExt};

use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_fetch::{FetchParams, FetchTool};

async fn server(status: &str, content_type: Option<&str>, body: &str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_string();
    let content_type = content_type.map(String::from);
    let body = body.to_string();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 4096];
        let _ = socket.read(&mut request).await;
        let header = content_type
            .map(|value| format!("Content-Type: {value}\r\n"))
            .unwrap_or_default();
        let response = format!(
            "HTTP/1.1 {status}\r\n{header}Content-Length: {}\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    format!("http://{address}/")
}

fn context() -> (ToolContext, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let backend = loopal_backend::LocalBackend::new(
        temp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "fetch-test",
    );
    (ToolContext::new(backend, "fetch-test"), temp)
}

fn tool() -> TypedBridge<FetchTool, FetchParams> {
    TypedBridge::new(FetchTool)
}

#[tokio::test]
async fn prompt_returns_converted_html_inline() {
    let url = server(
        "200 OK",
        Some("text/html"),
        "<html><body><h1>Title</h1><p>Body</p></body></html>",
    )
    .await;
    let (context, _temp) = context();

    let result = tool()
        .execute(
            serde_json::json!({"url": url, "prompt": "find title"}),
            &context,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("User prompt: find title"));
    assert!(result.content.contains("Title"));
}

#[tokio::test]
async fn no_prompt_downloads_into_workspace() {
    let body = r#"{"ok":true}"#;
    let url = server("200 OK", Some("application/json"), body).await;
    let (context, temp) = context();

    let result = tool()
        .execute(serde_json::json!({"url": url}), &context)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Content-Type: application/json"));
    assert!(
        result
            .content
            .contains(&format!("Size: {} bytes", body.len()))
    );
    let path = result
        .content
        .lines()
        .find_map(|line| line.strip_prefix("Downloaded to: "))
        .unwrap();
    assert!(path.ends_with(".json"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), body);
    assert!(
        std::path::Path::new(path)
            .canonicalize()
            .unwrap()
            .starts_with(temp.path().canonicalize().unwrap())
    );
}

#[tokio::test]
async fn backend_network_failure_is_a_tool_error_result() {
    let (context, _temp) = context();
    let result = tool()
        .execute(
            serde_json::json!({"url": "http://127.0.0.1:1/unreachable"}),
            &context,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(!result.content.is_empty());
}
