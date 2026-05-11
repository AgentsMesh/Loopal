use async_trait::async_trait;

use super::super::permission_handler::{PermissionHandler, PermissionOutcome};

pub struct DenyAllHandler;

#[async_trait]
impl PermissionHandler for DenyAllHandler {
    async fn decide(
        &self,
        _id: &str,
        _name: &str,
        _input: &serde_json::Value,
    ) -> PermissionOutcome {
        PermissionOutcome::deny("sub-agent context cannot prompt user")
    }
}
