#[derive(Serialize)]
struct AuditEntry<'a> {
    ts: String,
    op: SerializedOp,
    phase: &'static str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<&'a Path>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connection_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_digest: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_digest: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    intent_digest: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_run_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_node_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_attempt_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_phase: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision_source: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spawn_target: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_mode: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision_mode: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_policy: Option<&'a str>,
    pid: u32,
}
