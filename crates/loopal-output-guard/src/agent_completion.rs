use loopal_protocol::AgentCompletion;
use secrecy::SecretString;
use thiserror::Error;

use crate::{OutputGuard, OutputGuardBuildError, OutputGuardError};

pub const MAX_AGENT_COMPLETION_REASON_BYTES: usize = 256;
pub const MAX_AGENT_COMPLETION_RESULT_BYTES: usize = 100_000;
pub const OUTPUT_GUARD_REJECTED_REASON: &str = "output_guard_rejected";
pub const OUTPUT_GUARD_REJECTED_RESULT: &str = "agent completion rejected by output guard";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedAgentCompletion(AgentCompletion);

impl GuardedAgentCompletion {
    pub fn as_completion(&self) -> &AgentCompletion {
        &self.0
    }

    pub fn into_completion(self) -> AgentCompletion {
        self.0
    }
}

#[derive(Debug, Error)]
pub enum AgentCompletionGuardError {
    #[error("agent completion redactor unavailable")]
    Build(#[from] OutputGuardBuildError),
    #[error("agent completion reason rejected")]
    Reason(#[source] OutputGuardError),
    #[error("agent completion result rejected")]
    Result(#[source] OutputGuardError),
}

pub fn guard_agent_completion(
    completion: AgentCompletion,
    seed: &[(String, SecretString)],
) -> Result<GuardedAgentCompletion, AgentCompletionGuardError> {
    guard_agent_completion_with_result_limit(completion, seed, MAX_AGENT_COMPLETION_RESULT_BYTES)
}

pub fn guard_agent_completion_with_result_limit(
    completion: AgentCompletion,
    seed: &[(String, SecretString)],
    max_result_bytes: usize,
) -> Result<GuardedAgentCompletion, AgentCompletionGuardError> {
    let max_result_bytes =
        max_result_bytes.min(loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES as usize);
    if seed.is_empty() {
        enforce_completion_limits(&completion, max_result_bytes)?;
        return Ok(GuardedAgentCompletion(completion));
    }
    let guard = OutputGuard::new(seed)?;
    let reason = guard
        .guard_text(&completion.reason, MAX_AGENT_COMPLETION_REASON_BYTES)
        .map_err(AgentCompletionGuardError::Reason)?
        .into_inner()
        .into_string();
    let result = completion
        .result
        .as_deref()
        .map(|result| {
            guard
                .guard_text(result, max_result_bytes)
                .map(|guarded| guarded.into_inner().into_string())
                .map_err(AgentCompletionGuardError::Result)
        })
        .transpose()?;
    Ok(GuardedAgentCompletion(AgentCompletion::new(reason, result)))
}

fn enforce_completion_limits(
    completion: &AgentCompletion,
    max_result_bytes: usize,
) -> Result<(), AgentCompletionGuardError> {
    if completion.reason.len() > MAX_AGENT_COMPLETION_REASON_BYTES {
        return Err(AgentCompletionGuardError::Reason(
            OutputGuardError::ByteLimitExceeded {
                actual_bytes: completion.reason.len(),
                max_bytes: MAX_AGENT_COMPLETION_REASON_BYTES,
            },
        ));
    }
    if let Some(result) = completion.result.as_ref()
        && result.len() > max_result_bytes
    {
        return Err(AgentCompletionGuardError::Result(
            OutputGuardError::ByteLimitExceeded {
                actual_bytes: result.len(),
                max_bytes: max_result_bytes,
            },
        ));
    }
    Ok(())
}

pub fn guard_or_reject_agent_completion(
    completion: AgentCompletion,
    seed: &[(String, SecretString)],
) -> GuardedAgentCompletion {
    guard_or_reject_agent_completion_with_result_limit(
        completion,
        seed,
        MAX_AGENT_COMPLETION_RESULT_BYTES,
    )
}

pub fn guard_or_reject_agent_completion_with_result_limit(
    completion: AgentCompletion,
    seed: &[(String, SecretString)],
    max_result_bytes: usize,
) -> GuardedAgentCompletion {
    guard_agent_completion_with_result_limit(completion, seed, max_result_bytes)
        .unwrap_or_else(|_| GuardedAgentCompletion(rejected_agent_completion()))
}

pub fn rejected_agent_completion() -> AgentCompletion {
    AgentCompletion::new(
        OUTPUT_GUARD_REJECTED_REASON,
        Some(OUTPUT_GUARD_REJECTED_RESULT.into()),
    )
}
