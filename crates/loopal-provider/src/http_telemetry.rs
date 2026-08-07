use reqwest::StatusCode;
use tracing::{Span, field, info_span};

pub(crate) fn request_span(system: &'static str, endpoint: &str) -> Span {
    let server_address = endpoint_host(endpoint).unwrap_or_else(|| "unknown".into());
    info_span!(
        "http_request",
        otel.kind = "client",
        gen_ai.system = system,
        http.request.method = "POST",
        server.address = %server_address,
        http.response.status_code = field::Empty,
        error.type = field::Empty,
        otel.status_code = field::Empty,
        otel.status_message = field::Empty,
    )
}

pub(crate) fn record_response(span: &Span, status: StatusCode) {
    span.record("http.response.status_code", status.as_u16());
    if let Some(error_type) = response_error_type(status) {
        span.record("error.type", error_type.as_str());
        span.record("otel.status_code", "ERROR");
        let status_message = format!("HTTP {}", status.as_u16());
        span.record("otel.status_message", status_message.as_str());
    }
}

fn response_error_type(status: StatusCode) -> Option<String> {
    (!status.is_success()).then(|| status.as_u16().to_string())
}

pub(crate) fn record_transport_error(span: &Span) {
    span.record("error.type", "transport");
    span.record("otel.status_code", "ERROR");
    span.record("otel.status_message", "provider transport failure");
}

fn endpoint_host(endpoint: &str) -> Option<String> {
    reqwest::Url::parse(endpoint)
        .ok()?
        .host_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{endpoint_host, response_error_type};

    #[test]
    fn endpoint_host_drops_credentials_query_and_path() {
        assert_eq!(
            endpoint_host("https://user:secret@proxy.example/v1/responses?key=secret"),
            Some("proxy.example".into())
        );
    }

    #[test]
    fn response_error_type_is_low_cardinality_status_code() {
        assert_eq!(
            response_error_type(StatusCode::BAD_GATEWAY).as_deref(),
            Some("502")
        );
        assert_eq!(response_error_type(StatusCode::OK), None);
    }
}
