use std::path::Path;

use loopal_ipc::protocol::methods;
use loopal_protocol::{
    SecretCaller, SecretGetRequest, WorkflowAttemptCapability, WorkflowPermissionCausation,
    WorkflowProviderSecretGetRequest,
};

use crate::SecretError;

pub(super) enum SecretGetAuthority {
    Agent,
    WorkflowProvider {
        causation: WorkflowPermissionCausation,
        capability: WorkflowAttemptCapability,
    },
}

impl SecretGetAuthority {
    pub(super) fn request(
        &self,
        cwd: &Path,
        name: &str,
        caller: SecretCaller,
    ) -> Result<(&'static str, serde_json::Value), SecretError> {
        let (method, request) = match self {
            Self::Agent => (
                methods::HUB_SECRET_GET.name,
                serde_json::to_value(SecretGetRequest {
                    cwd: cwd.to_string_lossy().into_owned(),
                    name: name.to_string(),
                    caller,
                }),
            ),
            Self::WorkflowProvider {
                causation,
                capability,
            } => (
                methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name,
                serde_json::to_value(WorkflowProviderSecretGetRequest {
                    cwd: cwd.to_string_lossy().into_owned(),
                    name: name.to_string(),
                    causation: causation.clone(),
                    capability: capability.clone(),
                }),
            ),
        };
        request
            .map(|request| (method, request))
            .map_err(|error| SecretError::Ipc(format!("encode: {error}")))
    }
}
