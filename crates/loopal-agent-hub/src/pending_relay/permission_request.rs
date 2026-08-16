use loopal_protocol::{
    AgentEvent, AgentEventPayload, PermissionIntent, PermissionIntentRequest, PermissionIntentSeed,
    QualifiedAddress,
};

pub(super) struct PermissionRequest {
    pub(super) logical_id: String,
    pub(super) tool_name: String,
    tool_input: serde_json::Value,
    intent_seed: Option<PermissionIntentSeed>,
}

impl PermissionRequest {
    pub(super) fn parse(params: serde_json::Value) -> Result<Self, String> {
        if has_v2_field(&params) {
            let request: PermissionIntentRequest = serde_json::from_value(params)
                .map_err(|_| "invalid V2 permission request".to_string())?;
            request
                .validate()
                .map_err(|_| "invalid V2 permission request".to_string())?;
            return Ok(Self {
                logical_id: request.tool_call_id,
                tool_name: request.tool_name,
                tool_input: request.display_input,
                intent_seed: Some(request.intent_seed),
            });
        }
        let logical_id = required_string(&params, "tool_call_id")?;
        let tool_name = required_string(&params, "tool_name")?;
        let tool_input = params.get("tool_input").cloned().unwrap_or_default();
        Ok(Self {
            logical_id,
            tool_name,
            tool_input,
            intent_seed: None,
        })
    }

    pub(super) fn is_legacy(&self) -> bool {
        self.intent_seed.is_none()
    }

    pub(super) fn intent_seed(&self) -> Option<&PermissionIntentSeed> {
        self.intent_seed.as_ref()
    }

    pub(super) fn matches_workflow_authority(
        &self,
        expected: Option<&loopal_protocol::WorkflowPermissionCausation>,
    ) -> bool {
        self.intent_seed
            .as_ref()
            .is_some_and(|seed| seed.workflow() == expected)
    }

    pub(super) fn bind(
        &self,
        execution_generation: u64,
        ui_generation: u64,
        interaction_id: &str,
    ) -> Result<Option<PermissionIntent>, String> {
        self.intent_seed
            .clone()
            .map(|seed| {
                PermissionIntent::bind(seed, execution_generation, ui_generation, interaction_id)
                    .map_err(|_| "invalid V2 permission binding".to_string())
            })
            .transpose()
    }

    pub(super) fn event(
        &self,
        agent_name: &str,
        interaction_id: String,
        permission_intent: Option<PermissionIntent>,
    ) -> AgentEvent {
        AgentEvent::named(
            QualifiedAddress::local(agent_name),
            AgentEventPayload::ToolPermissionRequest {
                id: interaction_id,
                name: self.tool_name.clone(),
                input: self.tool_input.clone(),
                permission_intent: permission_intent.map(Box::new),
            },
        )
    }
}

fn has_v2_field(params: &serde_json::Value) -> bool {
    ["action_input", "tool_schema", "permission_intent"]
        .iter()
        .any(|key| params.get(key).is_some())
}

#[cfg(test)]
#[path = "permission_request_tests.rs"]
mod tests;

fn required_string(params: &serde_json::Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(String::from)
        .ok_or_else(|| format!("missing or empty {key}"))
}
