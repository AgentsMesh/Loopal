use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value;

const MAX_SCENARIO_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ExpectedRequest {
    pub protocol: Option<String>,
    pub model: Option<String>,
    pub user_contains: Option<String>,
    pub body_contains: Option<String>,
    pub body_excludes: Option<String>,
    pub tool_result_id: Option<String>,
    pub min_tools: Option<usize>,
    pub message_count: Option<usize>,
    pub assistant_block_types: Option<Vec<String>>,
    pub server_block_count: Option<usize>,
    pub thinking_enabled: Option<bool>,
    pub image_block_count: Option<usize>,
}

impl ExpectedRequest {
    pub fn is_empty(&self) -> bool {
        self.protocol.is_none()
            && self.model.is_none()
            && self.user_contains.is_none()
            && self.body_contains.is_none()
            && self.body_excludes.is_none()
            && self.tool_result_id.is_none()
            && self.min_tools.is_none()
            && self.message_count.is_none()
            && self.assistant_block_types.is_none()
            && self.server_block_count.is_none()
            && self.thinking_enabled.is_none()
            && self.image_block_count.is_none()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MockResponse {
    pub status: u16,
    pub retry_after_ms: Option<u64>,
    pub delay_ms: u64,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Value>,
    pub chunks: Vec<Value>,
    pub raw_sse: Vec<String>,
    pub disconnect_after_events: Option<usize>,
    pub close_before_headers: bool,
}

impl Default for MockResponse {
    fn default() -> Self {
        Self {
            status: 200,
            retry_after_ms: None,
            delay_ms: 0,
            headers: BTreeMap::new(),
            body: None,
            chunks: Vec::new(),
            raw_sse: Vec::new(),
            disconnect_after_events: None,
            close_before_headers: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MockCall {
    pub expected: ExpectedRequest,
    pub response: MockResponse,
}

#[derive(Debug)]
pub struct Scenario {
    pub name: String,
    calls: VecDeque<MockCall>,
    fallback: Option<MockCall>,
    served: usize,
}

impl Scenario {
    pub fn from_path(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("read scenario metadata: {}", path.display()))?;
        ensure!(
            metadata.len() <= MAX_SCENARIO_BYTES,
            "scenario exceeds 4 MiB"
        );
        let bytes =
            std::fs::read(path).with_context(|| format!("read scenario: {}", path.display()))?;
        Self::from_slice(&bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() as u64 <= MAX_SCENARIO_BYTES,
            "scenario exceeds 4 MiB"
        );
        let value: Value = serde_json::from_slice(bytes).context("parse scenario JSON")?;
        match value {
            Value::Array(calls) => Self::build("legacy".into(), calls, None),
            Value::Object(mut root) => {
                let version = root.remove("version").map_or(Ok(1), |value| {
                    value
                        .as_u64()
                        .context("scenario.version must be an integer")
                })?;
                ensure!(
                    (1..=2).contains(&version),
                    "unsupported scenario version {version}"
                );
                let name = root.remove("name").map_or(Ok("scenario".into()), |value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .context("scenario.name must be a string")
                })?;
                let calls = root
                    .remove("calls")
                    .and_then(|v| v.as_array().cloned())
                    .context("scenario.calls must be an array")?;
                let fallback = root
                    .remove("fallback")
                    .map(crate::scenario_parse::parse_call)
                    .transpose()?;
                ensure!(
                    root.is_empty(),
                    "unknown scenario field: {}",
                    root.keys().next().unwrap()
                );
                Self::build(name, calls, fallback)
            }
            _ => bail!("scenario root must be an object or array"),
        }
    }

    fn build(name: String, calls: Vec<Value>, fallback: Option<MockCall>) -> Result<Self> {
        ensure!(calls.len() <= 4096, "scenario has too many calls");
        let calls = calls
            .into_iter()
            .map(crate::scenario_parse::parse_call)
            .collect::<Result<VecDeque<_>>>()?;
        Ok(Self {
            name,
            calls,
            fallback,
            served: 0,
        })
    }

    pub fn take_matching(
        &mut self,
        body: &Value,
        record: &crate::RequestRecord,
    ) -> Option<MockCall> {
        let position = self
            .calls
            .iter()
            .position(|call| {
                !call.expected.is_empty()
                    && crate::validate_request(&call.expected, body, record).is_empty()
            })
            .or_else(|| self.calls.iter().position(|call| call.expected.is_empty()));
        let call = position
            .and_then(|index| self.calls.remove(index))
            .or_else(|| self.fallback.clone());
        if call.is_some() {
            self.served += 1;
        }
        call
    }

    pub fn served(&self) -> usize {
        self.served
    }
    pub fn remaining(&self) -> usize {
        self.calls.len()
    }
}
