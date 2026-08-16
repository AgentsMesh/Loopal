use std::collections::BTreeMap;

use loopal_protocol::{
    AgentCompletion, QualifiedAddress, WorkflowAgentNode, WorkflowAttemptFailure,
    WorkflowFailureClass, WorkflowLimits, WorkflowOutput, WorkflowOutputContract, WorkflowSpec,
    WorkflowWorkerProfileRef,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictAddress {
    pub hub: Vec<String>,
    pub agent: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictCompletion {
    pub reason: String,
    pub result: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrictFailureClass {
    TransientBeforeExecution,
    AmbiguousExecution,
    Permanent,
}

#[derive(Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum StrictOutput {
    Text(String),
    Json(serde_json::Value),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictFailure {
    pub class: StrictFailureClass,
    pub reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictSpec {
    pub version: u16,
    pub run_goal: String,
    pub nodes: Vec<StrictNode>,
    pub limits: StrictLimits,
    pub output_node: String,
    pub output_contract: StrictOutputContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictNode {
    pub id: String,
    pub dependencies: Vec<String>,
    pub task: String,
    pub worker_profile: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictLimits {
    pub max_nodes: u32,
    pub max_parallel: u32,
    pub max_attempts: u32,
    pub run_deadline_ms: u64,
    pub attempt_timeout_ms: u64,
    pub max_output_bytes: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum StrictOutputContract {
    Text {
        max_bytes: u32,
    },
    Json {
        max_bytes: u32,
        schema: BTreeMap<String, serde_json::Value>,
    },
}

impl From<StrictAddress> for QualifiedAddress {
    fn from(value: StrictAddress) -> Self {
        Self {
            hub: value.hub,
            agent: value.agent,
        }
    }
}

impl From<StrictCompletion> for AgentCompletion {
    fn from(value: StrictCompletion) -> Self {
        Self {
            reason: value.reason,
            result: value.result,
        }
    }
}

impl From<StrictOutput> for WorkflowOutput {
    fn from(value: StrictOutput) -> Self {
        match value {
            StrictOutput::Text(text) => Self::Text(text),
            StrictOutput::Json(json) => Self::Json(json),
        }
    }
}

impl From<StrictFailure> for WorkflowAttemptFailure {
    fn from(value: StrictFailure) -> Self {
        Self {
            class: value.class.into(),
            reason: value.reason,
        }
    }
}

impl From<StrictFailureClass> for WorkflowFailureClass {
    fn from(value: StrictFailureClass) -> Self {
        match value {
            StrictFailureClass::TransientBeforeExecution => Self::TransientBeforeExecution,
            StrictFailureClass::AmbiguousExecution => Self::AmbiguousExecution,
            StrictFailureClass::Permanent => Self::Permanent,
        }
    }
}

impl From<StrictSpec> for WorkflowSpec {
    fn from(value: StrictSpec) -> Self {
        Self {
            version: value.version,
            run_goal: value.run_goal,
            nodes: value.nodes.into_iter().map(Into::into).collect(),
            limits: value.limits.into(),
            output_node: value.output_node.into(),
            output_contract: value.output_contract.into(),
        }
    }
}

impl From<StrictNode> for WorkflowAgentNode {
    fn from(value: StrictNode) -> Self {
        Self {
            id: value.id.into(),
            dependencies: value.dependencies.into_iter().map(Into::into).collect(),
            task: value.task,
            worker_profile: WorkflowWorkerProfileRef::new(value.worker_profile),
        }
    }
}

impl From<StrictLimits> for WorkflowLimits {
    fn from(value: StrictLimits) -> Self {
        Self {
            max_nodes: value.max_nodes,
            max_parallel: value.max_parallel,
            max_attempts: value.max_attempts,
            run_deadline_ms: value.run_deadline_ms,
            attempt_timeout_ms: value.attempt_timeout_ms,
            max_output_bytes: value.max_output_bytes,
        }
    }
}

impl From<StrictOutputContract> for WorkflowOutputContract {
    fn from(value: StrictOutputContract) -> Self {
        match value {
            StrictOutputContract::Text { max_bytes } => Self::Text { max_bytes },
            StrictOutputContract::Json { max_bytes, schema } => Self::Json {
                max_bytes,
                schema: serde_json::Value::Object(schema.into_iter().collect()),
            },
        }
    }
}
