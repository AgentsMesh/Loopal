use std::sync::Arc;

use async_trait::async_trait;
use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_protocol::{PermissionDecisionAuditRequest, ProtectedEffectAuditRequest, ThreadGoal};
use loopal_secret_client::{
    HubHealth, IpcBudget, SecretClient, SecretError, SecretResult, SecretString,
};
use loopal_tool_api::{
    FetchRefinerPolicy, GoalSession, GoalSessionError, MemoryChannel, OneShotChatError,
    OneShotChatService, ProtectedEffectAudit, ToolContext,
};

pub struct Memory;
impl MemoryChannel for Memory {
    fn try_send(&self, _observation: String) -> Result<(), String> {
        Ok(())
    }
}

pub struct Chat;
#[async_trait]
impl OneShotChatService for Chat {
    async fn one_shot_chat(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        Ok("ok".into())
    }
}

pub struct Policy;
impl FetchRefinerPolicy for Policy {
    fn refiner_model(&self, _body_size: usize) -> Option<String> {
        Some("model".into())
    }
}

pub struct Goals;
#[async_trait]
impl GoalSession for Goals {
    async fn snapshot(&self) -> Result<Option<ThreadGoal>, GoalSessionError> {
        Ok(None)
    }
    async fn create(&self, _objective: String) -> Result<ThreadGoal, GoalSessionError> {
        Err(GoalSessionError::Storage("unused".into()))
    }
    async fn complete_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        Err(GoalSessionError::NotFound)
    }
    async fn reopen_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        Err(GoalSessionError::NotFound)
    }
    async fn mark_infeasible_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        Err(GoalSessionError::NotFound)
    }
}

pub struct EffectAudit;
#[async_trait]
impl ProtectedEffectAudit for EffectAudit {
    async fn record(&self, _request: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        Ok(())
    }

    async fn record_permission_decision(
        &self,
        _request: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        Ok(())
    }
}

pub struct Secrets;
#[async_trait]
impl SecretClient for Secrets {
    async fn get(&self, name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Err(SecretError::SecretNotFound(name.into()))
    }
    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(Vec::new())
    }
    async fn expand_author(
        &self,
        _template: &str,
        _budget: IpcBudget,
    ) -> SecretResult<SecretString> {
        Err(SecretError::TemplateParse("unused".into()))
    }
    async fn expand_wire(&self, _template: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Err(SecretError::TemplateParse("unused".into()))
    }
    fn health(&self) -> Option<Arc<HubHealth>> {
        None
    }
}

pub fn context() -> ToolContext {
    ToolContext::new(
        LocalBackend::new(
            std::env::temp_dir(),
            None,
            ResourceLimits::default(),
            "backend-session",
        ),
        "session",
    )
}
