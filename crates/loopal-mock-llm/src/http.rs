use std::collections::BTreeMap;

use anyhow::{Result, bail, ensure};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut data = Vec::with_capacity(8192);
    let header_end = loop {
        if let Some(index) = data.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        ensure!(data.len() < MAX_HEADER_BYTES, "HTTP headers too large");
        let mut chunk = [0u8; 8192];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            bail!("connection closed before HTTP headers");
        }
        data.extend_from_slice(&chunk[..read]);
    };
    let head = std::str::from_utf8(&data[..header_end])?;
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_owned();
    let path = request_line.next().unwrap_or_default().to_owned();
    ensure!(
        !method.is_empty() && path.starts_with('/'),
        "invalid HTTP request line"
    );
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            bail!("invalid HTTP header");
        };
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
    }
    ensure!(
        !headers.contains_key("transfer-encoding"),
        "chunked requests are unsupported"
    );
    let length = headers
        .get("content-length")
        .map_or(Ok(0), |value| value.parse::<usize>())?;
    ensure!(length <= MAX_BODY_BYTES, "HTTP body too large");
    while data.len() - header_end < length {
        let mut chunk = [0u8; 8192];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            bail!("connection closed before HTTP body");
        }
        data.extend_from_slice(&chunk[..read]);
    }
    Ok(HttpRequest {
        method,
        path,
        headers,
        body: data[header_end..header_end + length].to_vec(),
    })
}

pub async fn write_json(
    stream: &mut TcpStream,
    status: u16,
    body: &Value,
    headers: &BTreeMap<String, String>,
) -> Result<()> {
    write_response(
        stream,
        status,
        "application/json",
        body.to_string().as_bytes(),
        headers,
    )
    .await
}

pub async fn write_sse_head(
    stream: &mut TcpStream,
    headers: &BTreeMap<String, String>,
) -> Result<()> {
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n",
    );
    append_headers(&mut head, headers);
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    headers: &BTreeMap<String, String>,
) -> Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n",
        reason(status),
        body.len(),
    );
    append_headers(&mut head, headers);
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

fn append_headers(head: &mut String, headers: &BTreeMap<String, String>) {
    for (name, value) in headers {
        if name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !value.contains(['\r', '\n'])
        {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Mock Response",
    }
}
