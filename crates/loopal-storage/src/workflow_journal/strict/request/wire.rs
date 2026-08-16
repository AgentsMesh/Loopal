use serde::Deserialize;

use super::super::common::StrictSpec;
use super::super::snapshot::{StrictRunState, StrictSnapshot};
use super::StrictWaitStatus;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictStartRequest {
    pub request_id: String,
    pub spec: StrictSpec,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictStartResponse {
    pub summary: StrictSummary,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictGetRequest {
    pub request_id: String,
    pub run_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictGetResponse {
    pub run: Option<StrictSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictWaitRequest {
    pub request_id: String,
    pub run_id: String,
    pub after_revision: u64,
    pub timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictWaitResponse {
    pub status: StrictWaitStatus,
    pub run: Option<StrictSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictCancelRequest {
    pub request_id: String,
    pub run_id: String,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictCancelResponse {
    pub summary: StrictSummary,
    pub already_terminal: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictSummary {
    pub id: String,
    pub run_goal: String,
    pub state: StrictRunState,
    pub revision: u64,
    pub output_node: String,
    pub counts: StrictCounts,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StrictCounts {
    pub pending: u32,
    pub ready: u32,
    pub active: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub skipped: u32,
}
