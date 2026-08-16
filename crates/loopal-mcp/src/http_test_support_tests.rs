use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

pub(crate) struct RecordedRequest {
    pub method: String,
    pub token: Option<String>,
}

pub(crate) async fn server(responses: Vec<String>) -> (String, Arc<Mutex<Vec<RecordedRequest>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    tokio::spawn(async move {
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            let request_id = request
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
                .and_then(|value| value.get("id").cloned())
                .unwrap_or(serde_json::Value::Null);
            let response =
                refresh_content_length(response.replace("$REQUEST_ID", &request_id.to_string()));
            let mut lines = request.lines();
            let method = lines
                .next()
                .and_then(|line| line.split_whitespace().next())
                .unwrap_or_default()
                .to_string();
            let token = lines.find_map(|line| {
                line.strip_prefix("x-loopal-token: ")
                    .or_else(|| line.strip_prefix("X-Loopal-Token: "))
                    .map(str::to_string)
            });
            recorded
                .lock()
                .await
                .push(RecordedRequest { method, token });
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        }
    });
    (format!("http://{address}/mcp"), requests)
}

pub(crate) fn response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn refresh_content_length(response: String) -> String {
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return response;
    };
    let headers = headers
        .lines()
        .map(|line| {
            if line.to_ascii_lowercase().starts_with("content-length: ") {
                format!("Content-Length: {}", body.len())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    format!("{headers}\r\n\r\n{body}")
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let count = socket.read(&mut buffer).await.unwrap();
        if count == 0 {
            return String::from_utf8_lossy(&bytes).into_owned();
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header = String::from_utf8_lossy(&bytes[..header_end]);
    let content_length = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")?
                .parse()
                .ok()
        })
        .unwrap_or(0);
    while bytes.len() < header_end + content_length {
        let count = socket.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
