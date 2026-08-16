use loopal_output_guard::{FinalSinkRedactionSeed, MAX_AGENT_COMPLETION_REASON_BYTES, OutputGuard};
use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowEventPayload, WorkflowFailureClass,
    WorkflowOutput, WorkflowRunSnapshot,
};

use super::{AttemptKey, WorkflowSpawnFailure, WorkflowWorkerOutcome};

const MAX_FAILURE_REASON_BYTES: usize = 1_024;
const REJECTED_REASON: &str = "workflow_output_rejected";

pub(in crate::workflow) struct PreparedOutcome {
    pub(in crate::workflow) payload: WorkflowEventPayload,
}

pub(in crate::workflow) fn prepare_outcome(
    run: &WorkflowRunSnapshot,
    key: &AttemptKey,
    outcome: WorkflowWorkerOutcome,
    redaction_seed: &FinalSinkRedactionSeed,
) -> PreparedOutcome {
    match outcome {
        WorkflowWorkerOutcome::Succeeded { completion, output } => {
            prepare_success(run, key, completion, output, redaction_seed)
        }
        WorkflowWorkerOutcome::Failed(failure) => {
            prepare_spawn_failure(run, key, failure, redaction_seed)
        }
    }
}

pub(in crate::workflow) fn prepare_spawn_failure(
    run: &WorkflowRunSnapshot,
    key: &AttemptKey,
    failure: WorkflowSpawnFailure,
    redaction_seed: &FinalSinkRedactionSeed,
) -> PreparedOutcome {
    let Some((completion, reason)) = guard_failure(run, failure, redaction_seed) else {
        return rejected(key);
    };
    PreparedOutcome {
        payload: WorkflowEventPayload::AttemptFailed {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            completion,
            failure: reason,
        },
    }
}

fn prepare_success(
    run: &WorkflowRunSnapshot,
    key: &AttemptKey,
    completion: AgentCompletion,
    output: Option<WorkflowOutput>,
    redaction_seed: &FinalSinkRedactionSeed,
) -> PreparedOutcome {
    if !completion.is_success() {
        return rejected(key);
    }
    let Some((completion, output)) = guard_success(run, completion, output, redaction_seed) else {
        return rejected(key);
    };
    PreparedOutcome {
        payload: WorkflowEventPayload::AttemptSucceeded {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            completion,
            output,
        },
    }
}

fn guard_success(
    run: &WorkflowRunSnapshot,
    completion: AgentCompletion,
    output: Option<WorkflowOutput>,
    redaction_seed: &FinalSinkRedactionSeed,
) -> Option<(AgentCompletion, Option<WorkflowOutput>)> {
    let limit = run.spec.limits.max_output_bytes as usize;
    let snapshot = redaction_seed.snapshot().ok()?;
    let guard = OutputGuard::new(&snapshot).ok()?;
    let completion = guard_completion(&guard, completion, limit)?;
    let output = match output {
        // The output node's typed value is parsed from this same completion
        // result in the production spawner. Both representations are guarded,
        // but they share one byte allowance instead of consuming it twice.
        Some(value) => Some(guard_output(&guard, value, limit)?),
        None => None,
    };
    Some((completion, output))
}

fn guard_failure(
    run: &WorkflowRunSnapshot,
    failure: WorkflowSpawnFailure,
    redaction_seed: &FinalSinkRedactionSeed,
) -> Option<(AgentCompletion, WorkflowAttemptFailure)> {
    if failure.completion.is_success() {
        return None;
    }
    let snapshot = redaction_seed.snapshot().ok()?;
    let guard = OutputGuard::new(&snapshot).ok()?;
    let completion = guard_completion(
        &guard,
        failure.completion,
        run.spec.limits.max_output_bytes as usize,
    )?;
    let reason = guard
        .guard_text(&failure.failure.reason, MAX_FAILURE_REASON_BYTES)
        .ok()?
        .into_inner()
        .into_string();
    Some((
        completion,
        WorkflowAttemptFailure {
            class: failure.failure.class,
            reason,
        },
    ))
}

fn guard_completion(
    guard: &OutputGuard,
    completion: AgentCompletion,
    result_limit: usize,
) -> Option<AgentCompletion> {
    let reason = guard
        .guard_text(&completion.reason, MAX_AGENT_COMPLETION_REASON_BYTES)
        .ok()?
        .into_inner()
        .into_string();
    let result = completion
        .result
        .map(|value| {
            guard
                .guard_text(&value, result_limit)
                .map(|value| value.into_inner().into_string())
        })
        .transpose()
        .ok()?;
    Some(AgentCompletion::new(reason, result))
}

fn guard_output(
    guard: &OutputGuard,
    output: WorkflowOutput,
    limit: usize,
) -> Option<WorkflowOutput> {
    match output {
        WorkflowOutput::Text(value) => guard
            .guard_text(&value, limit)
            .ok()
            .map(|value| WorkflowOutput::Text(value.into_inner().into_string())),
        WorkflowOutput::Json(value) => guard
            .guard_json(&value, limit)
            .ok()
            .map(|value| WorkflowOutput::Json(value.into_inner().into_value())),
    }
}

fn rejected(key: &AttemptKey) -> PreparedOutcome {
    PreparedOutcome {
        payload: WorkflowEventPayload::AttemptFailed {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            completion: AgentCompletion::new(REJECTED_REASON, None),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "workflow worker output failed validation".into(),
            },
        },
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
