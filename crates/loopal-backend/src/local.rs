use std::path::PathBuf;
use std::sync::Arc;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::ResolvedPath;

use crate::approved::ApprovedPaths;
use crate::limits::ResourceLimits;
use crate::path;

pub struct LocalBackend {
    pub(crate) cwd: ResolvedPath,
    pub(crate) policy: Option<ResolvedPolicy>,
    pub(crate) limits: ResourceLimits,
    pub(crate) approved: ApprovedPaths,
    pub(crate) session_id: String,
}

impl LocalBackend {
    pub fn new(
        cwd: PathBuf,
        policy: Option<ResolvedPolicy>,
        limits: ResourceLimits,
        session_id: impl Into<String>,
    ) -> Arc<Self> {
        let canonical = path::strip_win_prefix(cwd.canonicalize().unwrap_or(cwd));
        Arc::new(Self {
            cwd: ResolvedPath::from_backend_resolved(canonical),
            policy,
            limits,
            approved: ApprovedPaths::new(),
            session_id: session_id.into(),
        })
    }

    pub(crate) fn resolve_checked(
        &self,
        raw: &str,
        is_write: bool,
    ) -> Result<PathBuf, ToolIoError> {
        match path::resolve(self.cwd.as_path(), raw, is_write, self.policy.as_ref()) {
            Ok(p) => Ok(p),
            Err(ToolIoError::RequiresApproval(reason)) => {
                let abs = path::to_absolute(self.cwd.as_path(), raw);
                if self.approved.contains(&abs) {
                    Ok(abs.canonicalize().unwrap_or(abs))
                } else {
                    Err(ToolIoError::RequiresApproval(reason))
                }
            }
            Err(e) => Err(e),
        }
    }
}
