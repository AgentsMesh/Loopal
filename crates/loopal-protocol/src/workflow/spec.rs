use std::fmt;

use serde::{Deserialize, Serialize};

use super::WorkflowNodeId;

pub const WORKFLOW_SPEC_V1: u16 = 1;
pub const MAX_WORKFLOW_NODES: u32 = 512;
pub const MAX_WORKFLOW_PARALLELISM: u32 = 64;
pub const MAX_WORKFLOW_ATTEMPTS: u32 = 2_048;
pub const MAX_WORKFLOW_RUN_DEADLINE_MS: u64 = 86_400_000;
pub const MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS: u64 = 14_400_000;
pub const MAX_WORKFLOW_OUTPUT_BYTES: u32 = 1_024 * 1_024;
pub const MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES: u64 = 8 * 1_024 * 1_024;
pub const MAX_WORKFLOW_GOAL_BYTES: usize = 32_768;
pub const MAX_WORKFLOW_TASK_BYTES: usize = 65_536;
pub const MAX_WORKFLOW_SPEC_BYTES: usize = 1_000_000;
pub const MAX_WORKER_PROFILE_BYTES: usize = 128;
pub const MAX_DEPENDENCIES_PER_NODE: usize = 64;
pub const MAX_JSON_SCHEMA_BYTES: usize = 65_536;
pub const MAX_JSON_SCHEMA_DEPTH: usize = 32;
pub const MAX_JSON_SCHEMA_NODES: usize = 2_048;
pub const MAX_JSON_SCHEMA_KEY_BYTES: usize = 256;
pub const MAX_JSON_SCHEMA_STRING_BYTES: usize = 16_384;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowWorkerProfileRef(String);

impl WorkflowWorkerProfileRef {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        self.0.len() <= MAX_WORKER_PROFILE_BYTES
            && self
                .0
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && self
                .0
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    }
}

impl fmt::Display for WorkflowWorkerProfileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSpec {
    pub version: u16,
    pub run_goal: String,
    pub nodes: Vec<WorkflowAgentNode>,
    pub limits: WorkflowLimits,
    pub output_node: WorkflowNodeId,
    pub output_contract: WorkflowOutputContract,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAgentNode {
    pub id: WorkflowNodeId,
    pub dependencies: Vec<WorkflowNodeId>,
    pub task: String,
    pub worker_profile: WorkflowWorkerProfileRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowLimits {
    pub max_nodes: u32,
    pub max_parallel: u32,
    pub max_attempts: u32,
    pub run_deadline_ms: u64,
    pub attempt_timeout_ms: u64,
    pub max_output_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowOutputContract {
    Text {
        max_bytes: u32,
    },
    Json {
        max_bytes: u32,
        schema: serde_json::Value,
    },
}

impl WorkflowOutputContract {
    pub fn max_bytes(&self) -> u32 {
        match self {
            Self::Text { max_bytes } | Self::Json { max_bytes, .. } => *max_bytes,
        }
    }
}
