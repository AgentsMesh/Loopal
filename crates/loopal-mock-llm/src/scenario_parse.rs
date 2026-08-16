use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;

use crate::{ExpectedRequest, MockCall, MockResponse};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallWire {
    label: Option<String>,
    #[serde(default, rename = "expect")]
    expected: ExpectedRequest,
    #[serde(flatten)]
    response: MockResponse,
}

pub(crate) fn parse_call(value: Value, version: u64) -> Result<MockCall> {
    if let Value::Array(chunks) = value {
        crate::schema::validate_chunks(&chunks)?;
        return Ok(MockCall {
            label: None,
            expected: ExpectedRequest::default(),
            response: MockResponse {
                chunks,
                ..MockResponse::default()
            },
        });
    }
    crate::schema::validate_call(&value)?;
    let wire: CallWire = serde_json::from_value(value).context("parse scenario call")?;
    if let Some(label) = wire.label.as_deref() {
        validate_label(label)?;
    }
    ensure!(
        version >= 3 || (wire.label.is_none() && wire.expected.request_metadata.is_none()),
        "call labels and requestMetadata require scenario version 3"
    );
    wire.expected.validate()?;
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
        label: wire.label,
        expected: wire.expected,
        response: wire.response,
    })
}

fn validate_label(label: &str) -> Result<()> {
    ensure!(!label.trim().is_empty(), "call label cannot be empty");
    ensure!(
        label.trim() == label,
        "call label cannot have outer whitespace"
    );
    ensure!(
        label.chars().count() <= 160,
        "call label cannot exceed 160 characters"
    );
    ensure!(
        !label.chars().any(char::is_control),
        "call label cannot contain control characters"
    );
    Ok(())
}
