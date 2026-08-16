use loopal_protocol::{WorkflowEvent, WorkflowEventPayload};
use serde::Deserialize;

use super::common::{StrictAddress, StrictCompletion, StrictFailure, StrictOutput};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictEvent {
    pub run_id: String,
    pub revision: u64,
    pub occurred_at_unix_ms: u64,
    pub payload: StrictEventPayload,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum StrictEventPayload {
    SpecValidated,
    RunStarted,
    RunDeadlineExceeded {
        failure: StrictFailure,
    },
    DispatchIntended {
        node_id: String,
        attempt_id: String,
        capability_digest: loopal_protocol::WorkflowAttemptCapabilityDigest,
    },
    AttemptBound {
        node_id: String,
        attempt_id: String,
        agent: StrictAddress,
    },
    AttemptRunning {
        node_id: String,
        attempt_id: String,
    },
    AttemptStopRequested {
        node_id: String,
        attempt_id: String,
        reason: String,
    },
    AttemptSucceeded {
        node_id: String,
        attempt_id: String,
        completion: StrictCompletion,
        output: Option<StrictOutput>,
    },
    AttemptFailed {
        node_id: String,
        attempt_id: String,
        completion: StrictCompletion,
        failure: StrictFailure,
    },
    CancelRequested {
        reason: Option<String>,
    },
    AttemptCancelled {
        node_id: String,
        attempt_id: String,
        reason: String,
    },
}

impl From<StrictEvent> for WorkflowEvent {
    fn from(value: StrictEvent) -> Self {
        Self {
            run_id: value.run_id.into(),
            revision: value.revision,
            occurred_at_unix_ms: value.occurred_at_unix_ms,
            payload: value.payload.into(),
        }
    }
}

impl From<StrictEventPayload> for WorkflowEventPayload {
    fn from(value: StrictEventPayload) -> Self {
        match value {
            StrictEventPayload::SpecValidated => Self::SpecValidated,
            StrictEventPayload::RunStarted => Self::RunStarted,
            StrictEventPayload::RunDeadlineExceeded { failure } => Self::RunDeadlineExceeded {
                failure: failure.into(),
            },
            StrictEventPayload::DispatchIntended {
                node_id,
                attempt_id,
                capability_digest,
            } => Self::DispatchIntended {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
                capability_digest,
            },
            StrictEventPayload::AttemptBound {
                node_id,
                attempt_id,
                agent,
            } => Self::AttemptBound {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
                agent: agent.into(),
            },
            StrictEventPayload::AttemptRunning {
                node_id,
                attempt_id,
            } => Self::AttemptRunning {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
            },
            StrictEventPayload::AttemptStopRequested {
                node_id,
                attempt_id,
                reason,
            } => Self::AttemptStopRequested {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
                reason,
            },
            StrictEventPayload::AttemptSucceeded {
                node_id,
                attempt_id,
                completion,
                output,
            } => Self::AttemptSucceeded {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
                completion: completion.into(),
                output: output.map(Into::into),
            },
            StrictEventPayload::AttemptFailed {
                node_id,
                attempt_id,
                completion,
                failure,
            } => Self::AttemptFailed {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
                completion: completion.into(),
                failure: failure.into(),
            },
            StrictEventPayload::CancelRequested { reason } => Self::CancelRequested { reason },
            StrictEventPayload::AttemptCancelled {
                node_id,
                attempt_id,
                reason,
            } => Self::AttemptCancelled {
                node_id: node_id.into(),
                attempt_id: attempt_id.into(),
                reason,
            },
        }
    }
}
