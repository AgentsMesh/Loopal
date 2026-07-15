use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;

use crate::{ExpectedRequest, MockCall, MockResponse};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallWire {
    #[serde(default, rename = "expect")]
    expected: ExpectedRequest,
    #[serde(flatten)]
    response: MockResponse,
}

pub(crate) fn parse_call(value: Value) -> Result<MockCall> {
    if let Value::Array(chunks) = value {
        crate::schema::validate_chunks(&chunks)?;
        return Ok(MockCall {
            expected: ExpectedRequest::default(),
            response: MockResponse {
                chunks,
                ..MockResponse::default()
            },
        });
    }
    crate::schema::validate_call(&value)?;
    let wire: CallWire = serde_json::from_value(value).context("parse scenario call")?;
    if let Some(protocol) = wire.expected.protocol.as_deref() {
        ensure!(
            matches!(
                protocol,
                "anthropic" | "openai_responses" | "openai_compat" | "google"
            ),
            "unsupported request protocol {protocol}"
        );
    }
    ensure!(
        (100..=599).contains(&wire.response.status),
        "invalid response status"
    );
    Ok(MockCall {
        expected: wire.expected,
        response: wire.response,
    })
}
