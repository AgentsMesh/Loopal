use async_trait::async_trait;
use loopal_protocol::PermissionIntentRequest;

use super::super::permission_handler::{PermissionHandler, PermissionOutcome};

pub struct DenyAllHandler;

#[async_trait]
impl PermissionHandler for DenyAllHandler {
    async fn decide(&self, _request: &PermissionIntentRequest) -> PermissionOutcome {
        PermissionOutcome::deny("sub-agent context cannot prompt user")
    }
}
