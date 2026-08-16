use loopal_protocol::{
    WorkflowAttemptId, WorkflowNodeId, WorkflowReduceError, WorkflowRequestError, WorkflowRunId,
    WorkflowValidationError, WorkflowWorkerProfileRef,
};
use loopal_workflow_schema::WorkflowSchemaError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowCoordinatorError {
    Disabled,
    Unavailable,
    InvalidOwner,
    OwnerPoisoned,
    RecoveryRequired,
    RecoveryInvalid,
    RecoveryConflict,
    JournalUnavailable,
    JournalLimit,
    CleanupTimeout,
    WaitTimeoutExceeded,
    InvalidRunId,
    InvalidGeneratedRunId(WorkflowRunId),
    InvalidGeneratedAttemptId(WorkflowAttemptId),
    AttemptIdCollision(WorkflowAttemptId),
    InvalidExecutionLease,
    StaleExecutionLease,
    RunDeadlineExceeded,
    TrustedLimitExceeded(&'static str),
    UnsupportedWorkerProfile {
        profile: WorkflowWorkerProfileRef,
    },
    UnsupportedWorkerProfileForNode {
        node_id: WorkflowNodeId,
        profile: WorkflowWorkerProfileRef,
    },
    RunIdCollision(WorkflowRunId),
    Request(WorkflowRequestError),
    Validation(WorkflowValidationError),
    Schema(WorkflowSchemaError),
    Reducer(WorkflowReduceError),
    UnexpectedStaleEvent,
    Encoding(String),
}

impl std::fmt::Display for WorkflowCoordinatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => f.write_str("workflow execution is disabled"),
            Self::Unavailable => f.write_str("workflow coordinator is unavailable"),
            Self::InvalidOwner => f.write_str("workflow owner is invalid"),
            Self::OwnerPoisoned => f.write_str("workflow owner requires coordinator restart"),
            Self::RecoveryRequired => f.write_str("workflow owner recovery is required"),
            Self::RecoveryInvalid => f.write_str("workflow recovery data is invalid"),
            Self::RecoveryConflict => f.write_str("workflow recovery conflicts with live state"),
            Self::JournalUnavailable => f.write_str("workflow journal is unavailable"),
            Self::JournalLimit => f.write_str("workflow journal limit exceeded"),
            Self::CleanupTimeout => f.write_str("workflow cleanup timed out"),
            Self::WaitTimeoutExceeded => write!(
                f,
                "workflow wait exceeds the {}ms limit",
                loopal_protocol::MAX_WORKFLOW_WAIT_MS
            ),
            Self::InvalidRunId => f.write_str("workflow run id is invalid"),
            Self::InvalidGeneratedRunId(id) => {
                write!(f, "generated workflow run id is invalid: {id}")
            }
            Self::InvalidGeneratedAttemptId(id) => {
                write!(f, "generated workflow attempt id is invalid: {id}")
            }
            Self::AttemptIdCollision(id) => {
                write!(f, "workflow attempt id already exists: {id}")
            }
            Self::InvalidExecutionLease => f.write_str("workflow execution lease is invalid"),
            Self::StaleExecutionLease => f.write_str("workflow execution lease is stale"),
            Self::RunDeadlineExceeded => f.write_str("workflow run deadline exceeded"),
            Self::TrustedLimitExceeded(field) => {
                write!(f, "workflow request exceeds configured {field}")
            }
            Self::UnsupportedWorkerProfile { profile } => {
                write!(f, "unsupported workflow worker profile: {profile}")
            }
            Self::UnsupportedWorkerProfileForNode { node_id, profile } => write!(
                f,
                "workflow node {node_id} uses unsupported worker profile: {profile}"
            ),
            Self::RunIdCollision(id) => write!(f, "workflow run id already exists: {id}"),
            Self::Request(error) => write!(f, "workflow request failed: {error:?}"),
            Self::Validation(error) => write!(f, "workflow validation failed: {error:?}"),
            Self::Schema(error) => write!(f, "workflow schema validation failed: {error}"),
            Self::Reducer(error) => write!(f, "workflow transition failed: {error:?}"),
            Self::UnexpectedStaleEvent => f.write_str("new workflow event was unexpectedly stale"),
            Self::Encoding(error) => write!(f, "workflow encoding failed: {error}"),
        }
    }
}

impl std::error::Error for WorkflowCoordinatorError {}

impl From<loopal_storage::WorkflowJournalError> for WorkflowCoordinatorError {
    fn from(error: loopal_storage::WorkflowJournalError) -> Self {
        match error {
            loopal_storage::WorkflowJournalError::LimitExceeded { .. } => Self::JournalLimit,
            loopal_storage::WorkflowJournalError::InvalidRunId(_)
            | loopal_storage::WorkflowJournalError::RunIdMismatch { .. }
            | loopal_storage::WorkflowJournalError::Corruption { .. }
            | loopal_storage::WorkflowJournalError::Serialization(_)
            | loopal_storage::WorkflowJournalError::RepairMismatch { .. } => Self::RecoveryInvalid,
            loopal_storage::WorkflowJournalError::Storage(_)
            | loopal_storage::WorkflowJournalError::Io { .. } => Self::JournalUnavailable,
        }
    }
}

impl From<WorkflowRequestError> for WorkflowCoordinatorError {
    fn from(error: WorkflowRequestError) -> Self {
        Self::Request(error)
    }
}

impl From<WorkflowValidationError> for WorkflowCoordinatorError {
    fn from(error: WorkflowValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<WorkflowSchemaError> for WorkflowCoordinatorError {
    fn from(error: WorkflowSchemaError) -> Self {
        Self::Schema(error)
    }
}

impl From<WorkflowReduceError> for WorkflowCoordinatorError {
    fn from(error: WorkflowReduceError) -> Self {
        Self::Reducer(error)
    }
}
