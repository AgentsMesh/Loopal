#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PermissionIntentSeedWire {
    version: u8,
    tool_name: String,
    action_digest: PermissionActionDigest,
    display_digest: PermissionDisplayDigest,
    schema_digest: PermissionSchemaDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<WorkflowPermissionCausation>,
}

impl TryFrom<PermissionIntentSeedWire> for PermissionIntentSeed {
    type Error = PermissionIntentError;

    fn try_from(value: PermissionIntentSeedWire) -> Result<Self, Self::Error> {
        let seed = Self {
            version: value.version,
            tool_name: value.tool_name,
            action_digest: value.action_digest,
            display_digest: value.display_digest,
            schema_digest: value.schema_digest,
            workflow: value.workflow,
        };
        seed.validate()?;
        Ok(seed)
    }
}

impl From<PermissionIntentSeed> for PermissionIntentSeedWire {
    fn from(value: PermissionIntentSeed) -> Self {
        Self {
            version: value.version,
            tool_name: value.tool_name,
            action_digest: value.action_digest,
            display_digest: value.display_digest,
            schema_digest: value.schema_digest,
            workflow: value.workflow,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PermissionIntentWire {
    version: u8,
    tool_name: String,
    action_digest: PermissionActionDigest,
    display_digest: PermissionDisplayDigest,
    schema_digest: PermissionSchemaDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow: Option<WorkflowPermissionCausation>,
    execution_generation: u64,
    ui_generation: u64,
    interaction_token: String,
    intent_digest: PermissionIntentDigest,
}

impl TryFrom<PermissionIntentWire> for PermissionIntent {
    type Error = PermissionIntentError;

    fn try_from(value: PermissionIntentWire) -> Result<Self, Self::Error> {
        let seed = PermissionIntentSeed::try_from(PermissionIntentSeedWire {
            version: value.version,
            tool_name: value.tool_name,
            action_digest: value.action_digest,
            display_digest: value.display_digest,
            schema_digest: value.schema_digest,
            workflow: value.workflow,
        })?;
        validate_binding(
            value.execution_generation,
            value.ui_generation,
            &value.interaction_token,
        )?;
        let expected = calculate_digest(
            &seed,
            value.execution_generation,
            value.ui_generation,
            &value.interaction_token,
        );
        if value.intent_digest != expected {
            return Err(PermissionIntentError::DigestMismatch);
        }
        Ok(Self {
            seed,
            execution_generation: value.execution_generation,
            ui_generation: value.ui_generation,
            interaction_token: value.interaction_token,
            intent_digest: value.intent_digest,
        })
    }
}

impl From<PermissionIntent> for PermissionIntentWire {
    fn from(value: PermissionIntent) -> Self {
        Self {
            version: value.seed.version,
            tool_name: value.seed.tool_name,
            action_digest: value.seed.action_digest,
            display_digest: value.seed.display_digest,
            schema_digest: value.seed.schema_digest,
            workflow: value.seed.workflow,
            execution_generation: value.execution_generation,
            ui_generation: value.ui_generation,
            interaction_token: value.interaction_token,
            intent_digest: value.intent_digest,
        }
    }
}

fn validate_binding(
    execution_generation: u64,
    ui_generation: u64,
    interaction_token: &str,
) -> Result<(), PermissionIntentError> {
    if execution_generation == 0 || ui_generation == 0 {
        return Err(PermissionIntentError::Generation);
    }
    if interaction_token.is_empty()
        || interaction_token.len() > MAX_INTERACTION_TOKEN_BYTES
        || interaction_token.chars().any(char::is_control)
    {
        return Err(PermissionIntentError::InteractionToken);
    }
    Ok(())
}

fn calculate_digest(
    seed: &PermissionIntentSeed,
    execution_generation: u64,
    ui_generation: u64,
    interaction_token: &str,
) -> PermissionIntentDigest {
    let version = [seed.version];
    let execution = execution_generation.to_be_bytes();
    let ui = ui_generation.to_be_bytes();
    let workflow = seed.workflow.as_ref();
    let workflow_marker = [u8::from(workflow.is_some())];
    let run = workflow.map_or("", |value| value.run_id.as_str());
    let node = workflow.map_or("", |value| value.node_id.as_str());
    let attempt = workflow.map_or("", |value| value.attempt_id.as_str());
    PermissionIntentDigest::from_bytes(framed_sha256(
        b"loopal.permission-intent.v2",
        &[
            &version,
            seed.tool_name.as_bytes(),
            seed.action_digest.as_bytes(),
            seed.display_digest.as_bytes(),
            seed.schema_digest.as_bytes(),
            &execution,
            &ui,
            interaction_token.as_bytes(),
            &workflow_marker,
            run.as_bytes(),
            node.as_bytes(),
            attempt.as_bytes(),
        ],
    ))
}
