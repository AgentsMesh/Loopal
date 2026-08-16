use std::fmt;

use super::WorkflowCoordinatorError;

#[derive(Debug)]
pub enum WorkflowRuntimeError {
    InvalidSettings(String),
    ProtectedAuditUnavailable,
    AlreadyAdmitted,
    AdmissionOccupied,
    Coordinator(WorkflowCoordinatorError),
    Tick(WorkflowCoordinatorError),
    TaskJoin { task: &'static str, message: String },
}

impl fmt::Display for WorkflowRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSettings(error) => write!(formatter, "invalid workflow settings: {error}"),
            Self::ProtectedAuditUnavailable => {
                formatter.write_str("workflow protected audit is unavailable")
            }
            Self::AlreadyAdmitted => formatter.write_str("workflow runtime is already admitted"),
            Self::AdmissionOccupied => {
                formatter.write_str("another workflow runtime is already admitted")
            }
            Self::Coordinator(error) => write!(formatter, "workflow coordinator failed: {error}"),
            Self::Tick(error) => write!(formatter, "workflow ticker failed: {error}"),
            Self::TaskJoin { task, message } => write!(formatter, "{task} task failed: {message}"),
        }
    }
}

impl std::error::Error for WorkflowRuntimeError {}
