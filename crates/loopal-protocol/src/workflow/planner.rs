use serde::{Deserialize, Serialize};

use super::{
    MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS, MAX_WORKFLOW_ATTEMPTS, MAX_WORKFLOW_NODES,
    MAX_WORKFLOW_OUTPUT_BYTES, MAX_WORKFLOW_PARALLELISM, MAX_WORKFLOW_RUN_DEADLINE_MS,
    WorkflowOutputContract, WorkflowSpec, WorkflowValidationError, validate_workflow_spec,
};

/// Version of the provider-neutral planner response contract.
pub const WORKFLOW_PLAN_V1: u16 = 1;
/// Planner responses are control data, not an unbounded model output channel.
pub const MAX_WORKFLOW_PLAN_BYTES: usize = 1_048_576;
/// The only execution choices a proactive planner can make.
///
/// This type intentionally contains no process, permission, sandbox, or
/// connection fields. A workflow choice is still only a candidate until the
/// Hub's typed `workflow/start` admission validates it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPlanDecision {
    pub version: u16,
    pub execution: WorkflowExecution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowExecution {
    Direct {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Workflow {
        spec: WorkflowSpec,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannerParseError {
    Empty,
    TooLarge,
    InvalidJson,
    InvalidShape,
    UnsupportedVersion(u16),
}

impl std::fmt::Display for PlannerParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("planner response is empty"),
            Self::TooLarge => f.write_str("planner response exceeds the byte limit"),
            Self::InvalidJson => f.write_str("planner response is not valid JSON"),
            Self::InvalidShape => f.write_str("planner response has an invalid shape"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported planner response version {version}")
            }
        }
    }
}

impl std::error::Error for PlannerParseError {}

/// Parse exactly one JSON planner response. Markdown fences, trailing prose,
/// unknown fields, and provider-specific extensions are rejected deliberately.
pub fn parse_workflow_plan(raw: &str) -> Result<WorkflowPlanDecision, PlannerParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PlannerParseError::Empty);
    }
    if trimmed.len() > MAX_WORKFLOW_PLAN_BYTES {
        return Err(PlannerParseError::TooLarge);
    }
    let decision: WorkflowPlanDecision =
        serde_json::from_str(trimmed).map_err(|_| PlannerParseError::InvalidJson)?;
    if decision.version != WORKFLOW_PLAN_V1 {
        return Err(PlannerParseError::UnsupportedVersion(decision.version));
    }
    Ok(decision)
}

/// Trusted limits copied from sanitized application settings. The protocol
/// owns this shape so validation stays independent of a settings crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowPlannerCeilings {
    pub max_nodes: u32,
    pub max_parallel: u32,
    pub max_attempts: u32,
    pub max_output_bytes: u32,
    pub run_deadline_ms: u64,
    pub attempt_timeout_ms: u64,
}

impl WorkflowPlannerCeilings {
    pub fn validate(&self) -> Result<(), PlannerLimitError> {
        let checks = [
            (
                self.max_nodes > 0 && self.max_nodes <= MAX_WORKFLOW_NODES,
                "max_nodes",
            ),
            (
                self.max_parallel > 0 && self.max_parallel <= MAX_WORKFLOW_PARALLELISM,
                "max_parallel",
            ),
            (
                self.max_attempts > 0 && self.max_attempts <= MAX_WORKFLOW_ATTEMPTS,
                "max_attempts",
            ),
            (
                self.max_output_bytes > 0 && self.max_output_bytes <= MAX_WORKFLOW_OUTPUT_BYTES,
                "max_output_bytes",
            ),
            (
                self.run_deadline_ms > 0 && self.run_deadline_ms <= MAX_WORKFLOW_RUN_DEADLINE_MS,
                "run_deadline_ms",
            ),
            (
                self.attempt_timeout_ms > 0
                    && self.attempt_timeout_ms <= MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS
                    && self.attempt_timeout_ms <= self.run_deadline_ms,
                "attempt_timeout_ms",
            ),
        ];
        checks
            .into_iter()
            .find_map(|(valid, field)| (!valid).then_some(PlannerLimitError::InvalidCeiling(field)))
            .map_or(Ok(()), Err)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannerLimitError {
    InvalidCeiling(&'static str),
    ExceedsCeiling(&'static str),
    InvalidSpec(WorkflowValidationError),
}

impl std::fmt::Display for PlannerLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCeiling(field) => write!(f, "invalid trusted ceiling {field}"),
            Self::ExceedsCeiling(field) => write!(f, "planner workflow exceeds ceiling {field}"),
            Self::InvalidSpec(error) => write!(f, "planner workflow is invalid: {error:?}"),
        }
    }
}

impl std::error::Error for PlannerLimitError {}

/// Cap planner-provided numeric limits, then run the canonical protocol
/// validator. Graph size is never truncated: a graph that does not fit is
/// rejected and must fall back to direct execution.
pub fn cap_and_validate_workflow(
    mut spec: WorkflowSpec,
    ceilings: WorkflowPlannerCeilings,
) -> Result<WorkflowSpec, PlannerLimitError> {
    ceilings.validate()?;
    if spec.nodes.len() > ceilings.max_nodes as usize {
        return Err(PlannerLimitError::ExceedsCeiling("max_nodes"));
    }
    spec.limits.max_nodes = spec.limits.max_nodes.min(ceilings.max_nodes);
    spec.limits.max_parallel = spec.limits.max_parallel.min(ceilings.max_parallel);
    spec.limits.max_attempts = spec.limits.max_attempts.min(ceilings.max_attempts);
    spec.limits.run_deadline_ms = spec.limits.run_deadline_ms.min(ceilings.run_deadline_ms);
    spec.limits.attempt_timeout_ms = spec
        .limits
        .attempt_timeout_ms
        .min(ceilings.attempt_timeout_ms);
    spec.limits.max_output_bytes = spec.limits.max_output_bytes.min(ceilings.max_output_bytes);
    match &mut spec.output_contract {
        WorkflowOutputContract::Text { max_bytes }
        | WorkflowOutputContract::Json { max_bytes, .. } => {
            *max_bytes = (*max_bytes).min(ceilings.max_output_bytes);
        }
    }
    validate_workflow_spec(&spec).map_err(PlannerLimitError::InvalidSpec)?;
    Ok(spec)
}
