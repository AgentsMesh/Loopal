//! Local HTTP callback server for OAuth redirect.
//!
//! Binds to `127.0.0.1:0` (ephemeral port), waits for a single GET request
//! with `code` and `state` query parameters, then returns them via channel.

use std::collections::HashMap;

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing::debug;

/// Parameters received from the OAuth callback.
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

/// Start a local HTTP server that receives one OAuth callback.
///
/// Returns `(port, receiver)`. The receiver completes when the browser
/// redirects back with the authorization code.
pub async fn start_callback_server() -> std::io::Result<(u16, oneshot::Receiver<CallbackParams>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let (tx, rx) = oneshot::channel();

    debug!(port, "OAuth callback server started");

    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0u8; 4096];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);

        if let Some(params) = parse_callback_params(&request) {
            let body = "<html><body><h2>Authorization successful!</h2>\
                <p>You can close this tab.</p></body></html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await;
            let _ = tx.send(params);
        } else {
            let body = "Missing code or state";
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await;
        }
    });

    Ok((port, rx))
}

fn parse_callback_params(request: &str) -> Option<CallbackParams> {
    let first_line = request.lines().next()?;
    // "GET /oauth_callback?code=xxx&state=yyy HTTP/1.1"
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((key.to_string(), percent_decode(value)))
        })
        .collect();

    Some(CallbackParams {
        code: params.get("code")?.clone(),
        state: params.get("state")?.clone(),
    })
}

/// Decode percent-encoded URL query values (e.g., `%2F` → `/`).
fn percent_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(hex_val);
            let lo = chars.next().and_then(hex_val);
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h << 4 | l) as char);
            } else {
                result.push('%');
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
