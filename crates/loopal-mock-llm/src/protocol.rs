use serde_json::{Value, json};

use crate::http::HttpRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireProtocol {
    Anthropic,
    OpenAiResponses,
    OpenAiCompat,
    Google,
}

impl WireProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiCompat => "openai_compat",
            Self::Google => "google",
        }
    }
}

pub(crate) struct Route {
    pub protocol: WireProtocol,
    pub model: Option<String>,
}

pub(crate) struct AuthMeta {
    pub key_present: bool,
    pub version_present: bool,
}

pub(crate) struct AuthFailure {
    pub status: u16,
    pub body: Value,
}

pub(crate) fn route(path: &str) -> Option<Route> {
    let bare = path.split('?').next().unwrap_or(path);
    let protocol = match bare {
        "/v1/messages" => WireProtocol::Anthropic,
        "/v1/responses" => WireProtocol::OpenAiResponses,
        "/v1/chat/completions" => WireProtocol::OpenAiCompat,
        _ => return google_route(bare),
    };
    Some(Route {
        protocol,
        model: None,
    })
}

fn google_route(path: &str) -> Option<Route> {
    let tail = path
        .strip_prefix("/models/")
        .or_else(|| path.strip_prefix("/v1beta/models/"))?;
    let model = tail.strip_suffix(":streamGenerateContent")?;
    (!model.is_empty()).then(|| Route {
        protocol: WireProtocol::Google,
        model: Some(model.to_owned()),
    })
}

pub(crate) fn authenticate(
    request: &HttpRequest,
    protocol: WireProtocol,
    expected_key: &str,
) -> Result<AuthMeta, AuthFailure> {
    match protocol {
        WireProtocol::Anthropic => anthropic_auth(request, expected_key),
        WireProtocol::OpenAiResponses | WireProtocol::OpenAiCompat => {
            bearer_auth(request, expected_key)
        }
        WireProtocol::Google => google_auth(request, expected_key),
    }
}

fn anthropic_auth(request: &HttpRequest, expected_key: &str) -> Result<AuthMeta, AuthFailure> {
    let key = request.headers.get("x-api-key");
    if key.map(String::as_str) != Some(expected_key) {
        return Err(unauthorized());
    }
    if request.headers.get("anthropic-version").map(String::as_str) != Some("2023-06-01") {
        return Err(AuthFailure {
            status: 400,
            body: json!({"error": "anthropic-version must be 2023-06-01"}),
        });
    }
    Ok(AuthMeta {
        key_present: true,
        version_present: true,
    })
}

fn bearer_auth(request: &HttpRequest, expected_key: &str) -> Result<AuthMeta, AuthFailure> {
    let header = request.headers.get("authorization");
    let expected = format!("Bearer {expected_key}");
    if header != Some(&expected) {
        return Err(unauthorized());
    }
    Ok(AuthMeta {
        key_present: true,
        version_present: true,
    })
}

fn google_auth(request: &HttpRequest, expected_key: &str) -> Result<AuthMeta, AuthFailure> {
    let query = request.path.split_once('?').map(|(_, query)| query);
    let key = query.and_then(|query| query_value(query, "key"));
    if key.as_deref() != Some(expected_key) {
        return Err(unauthorized());
    }
    let alt = query.and_then(|query| query_value(query, "alt"));
    if alt.as_deref() != Some("sse") {
        return Err(AuthFailure {
            status: 400,
            body: json!({"error": "Google streaming requires alt=sse"}),
        });
    }
    Ok(AuthMeta {
        key_present: true,
        version_present: true,
    })
}

fn query_value(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == name).then(|| percent_decode(value))
    })
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
        } else {
            output.push(if bytes[index] == b'+' {
                b' '
            } else {
                bytes[index]
            });
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn unauthorized() -> AuthFailure {
    AuthFailure {
        status: 401,
        body: json!({"error": {"type": "authentication_error"}}),
    }
}
