use std::path::PathBuf;

use loopal_error::ToolIoError;

use crate::path::ResolvedPath;

#[derive(Debug)]
pub enum BatchOp {
    Write {
        path: ResolvedPath,
        content: String,
        expected_kind: BatchWriteKind,
    },
    Delete {
        path: ResolvedPath,
    },
}

/// Plan-time prediction of write semantics. The backend trusts this and does
/// NOT re-stat to verify; if reality drifts (e.g. file removed between plan
/// and commit), `AppliedKind` may diverge from actual outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchWriteKind {
    Create,
    Update,
}

#[derive(Debug)]
pub struct BatchOutcome {
    pub applied: Vec<AppliedOp>,
    pub failed_at: Option<FailedOp>,
}

#[derive(Debug)]
pub struct AppliedOp {
    pub path: PathBuf,
    pub kind: AppliedKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedKind {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug)]
pub struct FailedOp {
    pub index: usize,
    pub path: PathBuf,
    pub error: ToolIoError,
}
